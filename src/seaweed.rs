use anyhow::{bail, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Permutation {
    image: Vec<usize>,
}

impl Permutation {
    pub fn identity(size: usize) -> Self {
        Self {
            image: (0..size).collect(),
        }
    }

    pub fn new(image: Vec<usize>) -> Result<Self> {
        let mut seen = vec![false; image.len()];
        for &value in &image {
            if value >= image.len() || seen[value] {
                bail!("invalid permutation image");
            }
            seen[value] = true;
        }
        Ok(Self { image })
    }

    pub fn len(&self) -> usize {
        self.image.len()
    }

    pub fn is_empty(&self) -> bool {
        self.image.is_empty()
    }

    pub fn apply(&self, index: usize) -> Option<usize> {
        self.image.get(index).copied()
    }

    pub fn compose(&self, rhs: &Self) -> Result<Self> {
        if self.len() != rhs.len() {
            bail!(
                "cannot compose permutations of different sizes: {} and {}",
                self.len(),
                rhs.len()
            );
        }

        let image = rhs
            .image
            .iter()
            .map(|&index| self.image[index])
            .collect::<Vec<_>>();
        Self::new(image)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeaweedMonoid {
    permutation: Permutation,
}

impl SeaweedMonoid {
    pub fn identity(size: usize) -> Self {
        Self {
            permutation: Permutation::identity(size),
        }
    }

    pub fn from_permutation(permutation: Permutation) -> Self {
        Self { permutation }
    }

    pub fn compose(&self, rhs: &Self) -> Result<Self> {
        Ok(Self {
            permutation: self.permutation.compose(&rhs.permutation)?,
        })
    }

    pub fn permutation(&self) -> &Permutation {
        &self.permutation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_composition_is_associative() {
        let a = SeaweedMonoid::from_permutation(Permutation::new(vec![1, 2, 0]).unwrap());
        let b = SeaweedMonoid::from_permutation(Permutation::new(vec![2, 0, 1]).unwrap());
        let c = SeaweedMonoid::from_permutation(Permutation::new(vec![0, 2, 1]).unwrap());

        let left = a.compose(&b).unwrap().compose(&c).unwrap();
        let right = a.compose(&b.compose(&c).unwrap()).unwrap();

        assert_eq!(left, right);
    }

    #[test]
    fn identity_is_neutral() {
        let item = SeaweedMonoid::from_permutation(Permutation::new(vec![2, 1, 0]).unwrap());
        let identity = SeaweedMonoid::identity(3);

        assert_eq!(item.compose(&identity).unwrap(), item);
        assert_eq!(identity.compose(&item).unwrap(), item);
    }
}
