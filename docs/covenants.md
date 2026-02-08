# Covenants and audit logging

Codex uses a versioned `covenant.json` file to declare which scopes are allowed for
explicit CLI actions such as command execution and patch application. The covenant
is required for intervention and proposal flows, and every action is logged to the
audit tables in the state database with the covenant version and scope.

## File location

Place `covenant.json` at the repository root. Codex looks for the file in the
current working directory and then walks parent directories until it finds a
versioned covenant.

## File shape

```json
{
  "version": "1.0",
  "scopes": [
    {
      "name": "exec",
      "capabilities": ["run_commands"]
    },
    {
      "name": "apply_patch",
      "capabilities": ["modify_workspace"]
    }
  ]
}
```

## Audit logging

When a proposal or intervention occurs, Codex writes an entry to `audit_actions`
that includes the timestamp, actor identity, action type, scope, covenant version,
and related event or intent identifiers. Actions outside the declared scopes are
refused and logged as blocked.
