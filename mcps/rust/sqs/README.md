# AWS SQS

The AWS SQS MCP provides an interface to interact with the AWS Simple Query Service through natural language queries.


## Core Tools

The MCP provides tools to List, Create and Delete queues, as well as to Send, Receive, and Delete messages.

Detailed documentation of the MCP interface is provided in the accompanying `.widl` file.

## Testing 

### Deployment
```
deploy -f <path to>/sqs.wasm -p <path to>/sqs.widl -c <path to>/config.yaml
```

#### `config.yaml`
```yaml
access_key_id: <Access key provided by AWS>
secret_access_key: <Secret Access Key, provided by AWS>
region: <AWS region in which the queues are>
```

### Prompt examples
The following prompts are mere suggestions; Icarus is very good in understanding what you mean.

- create a new queue named tests
- list my queues
   - Note: it takes sometime for the creation and deletion to be reflected in the listing.
- send the following messages to queue tests: "hello" "goodbye"
- receive 10 messages from queue tests and delete them. Show me the messages.
   - In this particular case, note that it is up to the queue to decide how many messages are actually received, so do not assume that the queue is empty if not enough messages are returned.
- delete the queue names `tests`

Queues may also be used in flows. For example:
- Example 1
   - 1. Flow: create queue number1
   - 2. Flow: create queue number2
   - 3. Flow: send message `obladi` to queue `number1`
- Example 2
   - 1. Flow: receive a message from queue `number1`
   - 2. Flow:  send the message received in the previous step to queue `number2`
   - 3. Flow: receive and delete a message from queue `number2`

Flows may involve other MCP, so, for example, one could receive a message from a queue and send it as an email message.

