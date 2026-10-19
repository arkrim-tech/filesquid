---
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Language-Agnostic Port Conflict Resolution

When the user encounters an error indicating that a network port is already in use (e.g., `EADDRINUSE`, `address already in use`), you MUST provide a framework-agnostic command the user can run to forcefully terminate the lingering process.

This happens frequently during development when server processes crash or fail to shut down gracefully, regardless of the technology stack (Node.js, Python, Elixir, Go, etc.).

## The Universal Command
Always suggest the following UNIX command to the user as a hint for them to run on their own machine, instructing them to replace `<your_port_number>` with the port number they are trying to free (e.g., 3000, 4000, 8080):

```bash
lsof -ti:<your_port_number> | xargs kill -9
```

**Agents MUST NOT run this command.** Other agents may share the machine and hold the port, and `lsof … | xargs kill -9` kills whatever holds it, not only your process. It is advice for the human, never a step the agent executes (see the Process rules in [filesquid-platform-engineer.md](../../.claude/agents/filesquid-platform-engineer.md)).

**Agent Behavior:**
1. An agent whose own server holds the port stops it by the PID it recorded when it started that server.
2. If the port is held by a process the agent did not start, the agent reports it to the human with the hint above, or uses a different free port where the task allows. It never kills that process.

**Rule Application:**
1. Do not assume the user knows how to clear a port just because they use a specific framework (e.g. Next.js, Phoenix).
2. Explicitly provide this `lsof` one-liner as a helpful hint whenever an `EADDRINUSE` failure is detected.
3. This is a stack-agnostic troubleshooting step that applies universally across all development environments on macOS/Linux.
