param(
    [string]$Corpus = ".",
    [string]$Pattern = "fn",
    [int]$Warmup = 3,
    [int]$Runs = 10
)

$ErrorActionPreference = "Stop"

cargo build --release --bin rocketgrep

hyperfine `
    --warmup $Warmup `
    --runs $Runs `
    "target/release/rocketgrep -F --color never '$Pattern' '$Corpus'" `
    "target/release/rocketgrep -k 1 --color never '$Pattern' '$Corpus'" `
    "rg -F --color never '$Pattern' '$Corpus'"

