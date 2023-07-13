# Generating Wiki

The docs folder will eventually move to [cloudwego.io](https://www.cloudwego.io/). You'll have to generate them locally for now.

## 1. Install Hugo

- Install a recent release of the Hugo "extended" version. If you install from the [Hugo release page](https://github.com/gohugoio/hugo/releases), make sure you download the `_extended` version which supports SCSS.

- If you have installed the latest version of Go, you can install directly by running the following command:
  ```
  go install -tags extended github.com/gohugoio/hugo@latest
  ```

- Alternatively, you can use the package manager to install Hugo:
  - For Linux: `sudo apt install hugo`
  - For macOS: `brew install hugo`

## 2. Run `wiki.sh`

- Execute the `wiki.sh` script, which performs the following tasks:
  - Checks out the [cloudwego/cloudwego.github.io](https://github.com/cloudwego/cloudwego.github.io) repository.
  - Copies the local `docs` folder to `content/en/docs/monolake/` within the cloned repository.
  - Starts the Hugo server for preview.
