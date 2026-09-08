## Purpose

Let approved local AI clients query useful history through stable interfaces without granting silent remote access or exposing more sensitive source data than requested.

## ADDED Requirements

### Requirement: Network access is local and authenticated
The HTTP interface SHALL bind only to loopback addresses by default and MUST reject requests without a valid application-issued credential. Remote binding SHALL not be available in the first release.

#### Scenario: Unauthenticated local request
- **WHEN** a process calls a protected history endpoint without valid credentials
- **THEN** the request is rejected without returning history metadata

#### Scenario: Request arrives from a non-loopback interface
- **WHEN** network traffic targets the server through a non-loopback address
- **THEN** no listener accepts the connection

### Requirement: MCP access is read-only and scoped
The MCP server SHALL expose search, recent-task, task-detail, daily-recap, and source-list operations as read-only tools or resources. Queries SHALL support explicit time ranges and result limits.

#### Scenario: Agent asks for recent work
- **WHEN** an authenticated MCP client requests recent tasks with a valid limit
- **THEN** the server returns summaries and permitted provenance without modifying history or executing actions

### Requirement: Least-sensitive responses are the default
Agent responses SHALL return derived summaries by default and SHALL omit raw event content, excluded intervals, credentials, and private-source details unless a separately exposed, explicitly approved scope allows them.

#### Scenario: Task contains private interval
- **WHEN** an agent retrieves a task adjacent to excluded activity
- **THEN** the response omits private content and reveals no excluded application or website identity

### Requirement: Captured content remains untrusted
The API and MCP output SHALL label source-derived text as untrusted evidence and MUST NOT convert embedded instructions into tool descriptions, prompts, or executable actions.

#### Scenario: Malicious instruction is present in source text
- **WHEN** a query result contains instruction-like captured content
- **THEN** the response preserves it only as data with provenance and an untrusted-content marker

### Requirement: Access can be revoked
The user SHALL be able to rotate credentials, revoke individual approved clients, disable the API, and disable MCP without stopping local collection.

#### Scenario: Client is revoked
- **WHEN** the user revokes an approved client
- **THEN** its existing credential fails on the next request while other approved clients remain unaffected
