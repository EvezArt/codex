<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install --cask codex</code></p>
<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

## Quickstart

### Installing and running Codex CLI

Install globally with your preferred package manager:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Team, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## HandshakeOS examples

The HandshakeOS CLI runs without direct hardware access; all inputs are user-provided data captured by the CLI or supplied as arguments in these examples.

### Initialize a workspace

```shell
handshakeos-e init
```

### Capture an observation with hypotheses

```shell
handshakeos-e capture \
  --intent "Route audio to Bluetooth" \
  --observation "Pressed play → audio routed to Bluetooth" \
  --hypothesis "Device auto-reconnects to last headset" --mixture 0.44 \
  --hypothesis "Media app forces BT output on play" --mixture 0.22 \
  --hypothesis "OS audio route preference set to Bluetooth" --mixture 0.17 \
  --hypothesis "Car mode profile activated" --mixture 0.09 \
  --hypothesis "Bluetooth audio sink is only available output" --mixture 0.08 \
  --test-result "Toggled Bluetooth off: audio stayed on speaker" \
  --evidence "trace://session/2025-02-14T09:31:04Z#step-3" \
  --outcome "Route preference is set to Bluetooth on play, but disabling Bluetooth falls back to speaker" \
  --outcome-evidence "trace://session/2025-02-14T09:31:04Z#step-4"
```

### Match patterns for a similar query

```shell
handshakeos-e patterns-match "Audio routes to Bluetooth when I hit play"
```

```text
Ranked matches:
1. Route audio to Bluetooth (score: 0.92)
2. Audio automatically switches to car stereo (score: 0.81)
3. Bluetooth headset reconnects on media start (score: 0.76)
```

## Ops Stack

This repository includes a comprehensive ops stack with canonical hashing support for data integrity testing. The ops-stack provides operational modules for market intelligence, notifications, automation, monetization, and AI engine management.

### Quick Start with Ops Stack

```shell
# Install dependencies
cd ops-stack
pnpm install

# Run golden hash tests
pnpm test

# Deploy (mock deployment with preflight checks)
cd ..
./deploy-ops-stack.sh
```

See [ops-stack/README.md](./ops-stack/README.md) for detailed documentation.

## GitHub Codespaces

This repository is optimized for GitHub Codespaces with a comprehensive devcontainer configuration that includes:

- **Languages & Tooling**: Node.js (v22), Rust, Go, Python 3, Java 21, Docker
- **Canonical Hashing Libraries**: json-canonicalize (npm), serde_jcs (Rust), webpki/jcs (Go), rfc8785 (Python), WebPKI/JCS (Java)
- **Development Tools**: Language servers, formatters, linters for all supported languages

### Opening in Codespaces

1. Click the green **Code** button on GitHub
2. Select **Codespaces** tab
3. Click **Create codespace on main**
4. Wait for the container to build (includes all language tooling)
5. Start developing!

### Running Tests in Codespaces

```shell
# Run ops-stack golden hash tests
cd ops-stack
pnpm test

# Run deployment script
cd ..
./deploy-ops-stack.sh
```

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)
- [**Ops Stack**](./ops-stack/README.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
