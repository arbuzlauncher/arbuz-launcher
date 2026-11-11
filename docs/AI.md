# AI assistant

The launcher uses AI in three places:

- crash diagnosis and fix suggestions;
- **AI pack builder** — picks mods, resource packs and shaders from a prompt;
- dependency review for an installed profile (**AI: dependencies** on the
  **Content** tab).

By default the built-in free **OpenCode Zen** provider is used — no API key is
required. You can connect an external provider with your own key.

## Where to configure

**Settings → AI pack assistant → AI provider**.

| Provider | Key | Default model |
| --- | --- | --- |
| OpenCode Zen | not required | selected automatically |
| OpenAI | `sk-…` | `gpt-4o-mini` |
| OpenRouter | `sk-or-v1-…` | `openrouter/auto` |
| Groq | `gsk_…` | `llama-3.3-70b-versatile` |
| DeepSeek | `sk-…` | `deepseek-chat` |
| Google Gemini | `AIza…` | `gemini-2.0-flash` |
| Anthropic Claude | `sk-ant-…` | `claude-3-5-sonnet-latest` |
| Custom server | optional | `gpt-4o-mini` |

**Test connection** sends a short request and shows the model response. The **↻**
button refreshes the provider model list; a model can also be typed manually if
it is not listed.

## Custom OpenAI-compatible server

Choose the **Custom server** provider and enter a Base URL without
`/chat/completions`:

| Server | Base URL |
| --- | --- |
| Ollama | `http://localhost:11434/v1` |
| LM Studio | `http://localhost:1234/v1` |
| vLLM | `http://localhost:8000/v1` |
| Your proxy | `https://ai.example.com/v1` |

`https://` is always allowed; `http://` is allowed only for `localhost`,
`127.0.0.1` and `[::1]`. The API key may be empty when the local server does not
require one.

## Key storage

The key is stored in `config.json` as `aiApiKey`. The webview receives it masked
as `************`, so saving settings does not overwrite the real key. The key is
excluded from settings export.

Keys are not shared between providers: switching the provider clears the key
field so a request cannot be sent with another provider's key.

## Configuration

```json
{
  "aiProvider": "openrouter",
  "aiApiKey": "sk-or-v1-...",
  "aiBaseUrl": "",
  "aiModel": "anthropic/claude-3.5-sonnet",
  "aiPackModel": "",
  "aiPackLimit": 0,
  "aiBuilder": true
}
```

| Field | Meaning |
| --- | --- |
| `aiProvider` | `opencode`, `openai`, `openrouter`, `groq`, `deepseek`, `gemini`, `anthropic`, `custom` |
| `aiApiKey` | provider key |
| `aiBaseUrl` | Base URL for `custom` |
| `aiModel` | model for crash diagnosis and dependency review; empty means default |
| `aiPackModel` | model for the AI pack builder; empty falls back to `aiModel` or the provider default |
| `aiPackLimit` | maximum number of selected mods, `0` means no limit |
| `aiBuilder` | enables the AI pack builder UI |

## Commands

| Command | Purpose |
| --- | --- |
| `ai_models` | list models of the selected provider |
| `ai_test` | short test request |
| `ai_diagnose_crash` | crash diagnosis |
| `ai_pack_plan` | AI pack plan |
| `ai_dependency_plan` | compatible updates and pack issues |
| `ai_scan_note` | explanation for one suspicious mod |

## Privacy

- Requests include only the game version, loader, mod list and a fragment of
  the crash report; paths containing the user name are replaced with `<user>`.
- The proxy configured in settings is also used for AI requests.
- Plugins can extend the dependency plan through the `ai.dependencies.plan`
  hook and observe requests through `ai.*` hooks (see `docs/PLUGINS.md`).
