# SNAS Socket Protocol

SNAS primarily uses a NATS API. However, when running on a client system, it is easier for something like a PAM module to call a local socket as opposed to doing a NATS connection (which also requires local credentials or a leaf node, both of which could be a security issue). For this purpose, SNAS by default will listen on a Unix Domain Socket, owned by the user running SNAS (probably root), to serve requests for what is called the "user" API. This API is relatively simple as it consists of a method to validate a user's credentials and return its groups and a method for changing the user's password. 

This document describes the lightweight protocol used by SNAS to communicate with the socket.

## Protocol

Each request MUST be written to the socket in the following format:

```
REQ\n<method>\n<json-data>\r\nEND\n
```

The response must be written to the socket in the following format:

```
RES\n<json-data>\r\nEND\n
```

The server does not make any guarantees about request timeout for partially written requests (i.e. it can wait for any length of time for the rest of a request to be written), but it MUST send a response if it does choose to timeout.

A client MAY keep a connection to the socket open and issue multiple requests over it. However, the client MUST NOT write another request until a response is received. The server MUST respond to each request in the order it receives them for that connection.

## Methods

### `verify`

The `verify` method is used to verify a user's credentials. It takes a JSON object with the following fields:

```json
{
    "username": "username",
    "password": "password",
}
```

The response will be a JSON object with the following fields:

```json
{
    "success": true | false,
    "message": "a message",
    "response": {
        "valid": true | false,
        "message": "a message with additional context",
        "needs_password_reset": true | false,
        "groups": ["list", "of", "groups"],
    }
}
```

### `change_password`

The `change_password` method is used to change a user's password. It takes a JSON object with the following fields:

```json
{
    "username": "username",
    "old_password": "old password",
    "new_password": "new password",
}
```

The response will be a JSON object with the following fields:

```json
{
    "success": true | false,
    "message": "a message"
}
```
