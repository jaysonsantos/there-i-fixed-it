# There, I fixed it!

[![pre-commit.ci status](https://results.pre-commit.ci/badge/github/jaysonsantos/there-i-fixed-it/main.svg)](https://results.pre-commit.ci/badge/github/jaysonsantos/there-i-fixed-it/main.svg)

This is a tool to automate repetitive tasks on repositories of an organization.

## What it does

When you supply a plan, the process is the following:

- Get a list of repositories from your organization.
- Apply the repositoris and deny repositories filters
- Clone them all on the cache folder
- Change to default branch
- Pull changes (as this is cached, better to be safe that it has the latest changes)
- Checkout to your desired branch
- Iterate over the files and run all processors
- Commit the changes
- Push
- Open a pull request
- Profit!

## Usage

```
there-i-fixed-it 0.1.0

USAGE:
    there-i-fixed-it [FLAGS] --plan-file <plan-file>

FLAGS:
    -h, --help                     Prints help information
    -s, --skip-repository-cache
    -V, --version                  Prints version information

OPTIONS:
    -f, --plan-file <plan-file>
```

Example of a plan:

```toml
branch_name = "automated/update-flag"
git_message = "chore: Update flag that should be false"
pull_request_title = "Update flag that should be false" # Optional, if missing git_message is used
pull_request_body = "This updates the flag that should be false @jaysonsantos."
repositories = ["my-repo"] # Also works with globs like python-*, *-rs, or *
deny_repositories = [
] # Optional, if present it runs after the above filter to remove denied repositories

[provider]
name = "github" # Only github is implemented but others should be easy to implement
user = "user-name"
token = "token"
organization = "my-organization"

[[files]]
glob = "terraform/**/*.tf"
processors = [
    { type = "regex", operations = [
        { from = "(delete_everything\\W+=\\W+)true", to = "${1}false" },
    ] }
]

# You can have multiple [[files]]
[[files]]
glob = "**.py"
processors = [
    { type = "regex", operations = [
        { from = "(def\\W+)wrong_function_name", to = "${1}right_function_name" }
    ] }
]
```

## Disclaimer

No warranties!
