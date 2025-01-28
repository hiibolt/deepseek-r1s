# deepseek-r1s
![image](https://github.com/user-attachments/assets/0837d8fb-b7c6-4fbc-870c-149576609587)
### About
Highly portable full-stack DeepSeek-r1 backend + frontend, allowing you to host the model and toy with it on your own system without the need for complex installations. 

**Features**:
* **Streaming** - Responses are streamed as they are generated
* **Parsed Thoughts** - Thoughts are contained seperately visually to allow users to see the process, and are unexpanded on the final result
* **Message History** - Retains messages throughout conversations
* **Asynchronus** - You can hold multiple conversations with the model simultaneously
* **Markdown and LaTeX Support** - Ask it about programming, math, and more - all without having to know special syntax

## Usage
### Docker Compose
Clone this repository and start the stack:
```bash
git clone https://github.com/hiibolt/deepseek-r1s.git
cd deepseek-r1s
docker compose up
```
### From Source
Requires the [Nix Package Manager](https://nixos.org/) with [Nix Flakes](https://wiki.nixos.org/wiki/Flakes) enabled.

Clone this repository:
```bash
git clone https://github.com/hiibolt/deepseek-r1s.git
cd deepseek-r1s
```

Enter its development environment:
```bash
nix develop .#deepseek-r1s
```

Set your environment variables:
```bash
export MODEL_NAME='deepseek-r1:1.5b'
export DATA_DIR_PATH='./data'
```

Start the backend, and visit `localhost:5776`!
```bash
cargo run
```
