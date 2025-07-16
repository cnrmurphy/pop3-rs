# POP3 Server

A POP3 (Post Office Protocol version 3) server implementation in Rust, compliant with RFC 1939.

## Description

This project implements a POP3 server that supports user authentication, mailbox access, and message retrieval using the Maildir format for mail storage. The server features a type-safe session state machine that ensures protocol compliance and prevents common implementation errors.

## Features

- **Maildir Support**: Uses standard Maildir format for mail storage
- **User Authentication**: Argon2-based password hashing with salt
- **Session Management**: Concurrent session handling with per-user mailbox locking
- **Type-Safe State Machine**: Prevents invalid state transitions at compile time
- **CLI User Management**: Built-in command-line interface for creating users

## Quick Start

1. **Build the project:**
   ```bash
   cargo build --release
   ```

2. **Create a user:**
   ```bash
   ./target/release/server add-user alice password123
   ```

3. **Start the server:**
   ```bash
   cargo run
   ```

4. **Connect via telnet:**
   ```bash
   telnet localhost 1110
   ```

## POP3 Command Implementation Status

### ✅ Implemented Commands

- **USER** `<username>` - Specify username for authentication
- **PASS** `<password>` - Provide password for authentication  
- **LIST** - List messages with sizes
- **RETR** `<msg#>` - Retrieve a specific message
- **DELE** `<msg#>` - Mark message for deletion
- **NOOP** - No operation (keep connection alive)
- **QUIT** - Close connection and commit deletions

### 🚧 In Progress

- **DELE** - Message deletion marking (partial implementation)

### ❌ Not Yet Implemented

#### Required by RFC 1939
- **STAT** - Get mailbox statistics (message count, total size)
- **UIDL** `[msg#]` - Get unique message identifiers
- **RSET** - Reset session (unmark deleted messages)
- **TOP** `<msg#> <lines>` - Get message headers + first N lines

#### Optional Extensions
- **APOP** `<name> <digest>` - Secure authentication via MD5 digest
- **CAPA** - List server capabilities

## Architecture

### Core Components

- **Session State Machine**: Type-safe states (Authorization, Transaction, Update)
- **MailDir Abstraction**: Filesystem operations for mail storage
- **Authentication Store**: Sled-based user credential storage
- **Session Manager**: Concurrent session handling with mailbox locking

### Session States

1. **Authorization**: User provides credentials
2. **Transaction**: Authenticated user can read/delete messages
3. **Update**: Apply deletions and clean up resources

### Security Features

- Argon2 password hashing with random salts
- Per-user mailbox locking prevents concurrent access
- Input validation and sanitization
- Generic error messages to prevent information leakage

## Testing

```bash
# Run unit tests
cargo test

# Run with verbose output
cargo test -- --nocapture
```

## Configuration

- **Server Port**: 1110 (configured in main.rs)
- **Database Path**: `my_db/` (Sled database)
- **Mail Storage**: `Maildir/` (Maildir format)

## RFC Compliance

This implementation aims for full RFC 1939 compliance. The server correctly implements:

- Three-state protocol machine (AUTHORIZATION → TRANSACTION → UPDATE)
- Proper response codes (`+OK` and `-ERR`)
- Message numbering (1-based indexing)
- Mailbox locking during transactions
- Atomic message deletion on QUIT

## Contributing

Feel free to contribute by implementing missing commands or improving existing functionality. Please ensure all changes maintain RFC 1939 compliance.

## License

This project is for educational purposes.