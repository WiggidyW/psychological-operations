# psychological-operations


An agent that uses computer use to browse [X](https://x.com), searching for tweets that match user-defined criteria. It finds tweets worth replying to by scoring and ranking them through a two-pass analysis pipeline powered by [ObjectiveAI](https://github.com/ObjectiveAI/objectiveai).

## How it works

1. **Search** — The agent browses X via computer use, searching for tweets matching provided keywords.

2. **First pass (cheap)** — Every tweet found is scored using an ObjectiveAI Vector Function with a cheap profile. This filters out noise quickly and affordably.

3. **Second pass (expensive)** — The top N% of tweets from the first pass are re-analyzed using an ObjectiveAI Vector Function with an expensive profile, producing a final ranking.

4. **Results** — The highest-scoring tweets are surfaced to the user as candidates to reply to.

The ObjectiveAI Function is invented (generated) based on user-specified criteria describing what kind of tweets they want to find. The function scores and ranks tweets as vectors, so results are ordered by relevance to the user's goals.

## Profile improvement

Profiles can be improved over time through feedback. When the user indicates which results were good or bad, the ObjectiveAI profile computation endpoint refines the profile to better identify the type of tweets the user is looking for. This uses ObjectiveAI credits.

## System requirements

- **Windows**: Windows 10 build 17063 or later (required for Unix domain socket support)
- **macOS / Linux**: Any modern version

## LLM credits

The agent's LLM usage (for computer use and tweet analysis) can be funded through:

- **OpenRouter credits** — via OpenRouter API key
- **Anthropic credits** — via Anthropic API key
- **Anthropic subscription** — only when using a locally-running ObjectiveAI API server
