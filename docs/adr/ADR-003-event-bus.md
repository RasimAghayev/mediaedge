# ADR-003: Event Bus

## Status
Proposed

## Context
Durable, replayable event bus with exactly-once semantics for replication,
media processing, and edge sync. Options: NATS JetStream, Kafka, Redis Streams.

## Decision
Use NATS JetStream. Built-in geo-replication, exactly-once delivery, stream
replay, and low operational overhead.

## Consequences
- `async-nats` dependency
- JetStream stream/consumer configuration required
