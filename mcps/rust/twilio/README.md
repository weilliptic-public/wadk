# Twilio MCP Server
A Model Context Protocol (MCP) applet that integrates with the **Twilio REST API (2010-04-01)** to send SMS messages, place voice calls
via TwiML, fetch recent messages, and retrieve account information.

## Core tools
- `send_sms`: Create an outbound SMS via `POST /Messages.json`.
- `send_voice_note`: Initiate a voice call with a TwiML URL via `POST /Calls.json`.
- `get_messages`: List recent messages via `GET /Messages.json` (optional `PageSize`).
- `get_account_info`: Fetch account metadata via `GET /Accounts/{sid}.json`.


## Testing 

### Deployment
```
deploy -f <path to>/twilio.wasm -p <path to>/twilio.widl -c <path to>/config.yaml
```

#### `config.yaml`
```yaml
twilio_account_sid: <YOUR ACCOUNT SID>
twilio_auth_token: <YOUR AUTH TOKEN>
twilio_phone_number: "<INTERNATIONAL NUMBER>"
```

### Prompt examples

- Get all SMS messages from twilio
- Check my Twilio account information
- Show account status and details
- Send SMS from +18012345678 to +9112345678 saying "Hello World" using twilio tools with twilio
- Send an SMS reminder saying you have an AI Conference tomorrow from number +112345678 to +912345678 using twilio tools
