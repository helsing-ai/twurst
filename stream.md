# Twurst stream format

Twurst implements streaming on top of Twirp.

The streams are transferred using HTTP chunked transfer encoding.

The `content-type`s header value must be:
- `application/x-twurst-protobuf-stream`
- `application/jsonl`

### `application/x-twurst-protobuf-stream`
The stream format is a sequence of:
- 1 byte that is 0 for a message and 48 for an error.
- 4 bytes that is the *size* of the following message as an unsigned 32 bits big endian integer
- *size* bytes that is the message encoded in protobuf or the Twirp error encoded in JSON (based on the first byte value)


### `application/jsonl`
The stream format is a sequence of JSON trees under the [JSON lines format](https://jsonlines.org/).
Each line is either `{"message": M}\n` with `M` the response message or `{"error": E}\n` with `E` a Twirp error in JSON.
