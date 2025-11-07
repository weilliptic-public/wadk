package contract

import (
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)


func coreListUsers() (*[]SlackUser, errors.WeilError) {
    state := runtime.State[SlackContractState]()
    result, err := state.ListUsers()

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("ListUsers", err)
    }

    return result, nil
}

func ListUsers() {
    var resp *types.Result[[]SlackUser, errors.WeilError]

    result, err := coreListUsers()

    if err != nil {
        resp = types.NewErrResult[[]SlackUser, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[[]SlackUser, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreListPublicChannels() (*[]Channel, errors.WeilError) {
    state := runtime.State[SlackContractState]()
    result, err := state.ListPublicChannels()

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("ListPublicChannels", err)
    }

    return result, nil
}

func ListPublicChannels() {
    var resp *types.Result[[]Channel, errors.WeilError]

    result, err := coreListPublicChannels()

    if err != nil {
        resp = types.NewErrResult[[]Channel, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[[]Channel, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreListPrivateChannels() (*[]Channel, errors.WeilError) {
    state := runtime.State[SlackContractState]()
    result, err := state.ListPrivateChannels()

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("ListPrivateChannels", err)
    }

    return result, nil
}

func ListPrivateChannels() {
    var resp *types.Result[[]Channel, errors.WeilError]

    result, err := coreListPrivateChannels()

    if err != nil {
        resp = types.NewErrResult[[]Channel, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[[]Channel, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreGetConversationsFromChannel() (*[]Message, errors.WeilError) {
    type GetConversationsFromChannelArgs struct {
        ChannelName string `json:"channel_name"`
    }

    state, args, err := runtime.StateAndArgs[SlackContractState, GetConversationsFromChannelArgs]()

    if err != nil {
        return nil, errors.NewMethodArgumentDeserializationError("GetConversationsFromChannel", err)
    }

    result, err := state.GetConversationsFromChannel(args.ChannelName)

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("GetConversationsFromChannel", err)
    }

    return result, nil
}

func GetConversationsFromChannel() {
    var resp *types.Result[[]Message, errors.WeilError]

    result, err := coreGetConversationsFromChannel()

    if err != nil {
        resp = types.NewErrResult[[]Message, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[[]Message, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreGetConversationsWithUser() (*[]Message, errors.WeilError) {
    type GetConversationsWithUserArgs struct {
        UserName string `json:"user_name"`
    }

    state, args, err := runtime.StateAndArgs[SlackContractState, GetConversationsWithUserArgs]()

    if err != nil {
        return nil, errors.NewMethodArgumentDeserializationError("GetConversationsWithUser", err)
    }

    result, err := state.GetConversationsWithUser(args.UserName)

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("GetConversationsWithUser", err)
    }

    return result, nil
}

func GetConversationsWithUser() {
    var resp *types.Result[[]Message, errors.WeilError]

    result, err := coreGetConversationsWithUser()

    if err != nil {
        resp = types.NewErrResult[[]Message, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[[]Message, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreSendMessageToChannel() (*string, errors.WeilError) {
    type SendMessageToChannelArgs struct {
        ChannelName string `json:"channel_name"`
        Message string `json:"message"`
    }

    state, args, err := runtime.StateAndArgs[SlackContractState, SendMessageToChannelArgs]()

    if err != nil {
        return nil, errors.NewMethodArgumentDeserializationError("SendMessageToChannel", err)
    }

    result, err := state.SendMessageToChannel(args.ChannelName, args.Message)

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("SendMessageToChannel", err)
    }

    return result, nil
}

func SendMessageToChannel() {
    var resp *types.Result[string, errors.WeilError]

    result, err := coreSendMessageToChannel()

    if err != nil {
        resp = types.NewErrResult[string, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[string, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

func coreSendMessageToUser() (*string, errors.WeilError) {
    type SendMessageToUserArgs struct {
        UserName string `json:"user_name"`
        Message string `json:"message"`
    }

    state, args, err := runtime.StateAndArgs[SlackContractState, SendMessageToUserArgs]()

    if err != nil {
        return nil, errors.NewMethodArgumentDeserializationError("SendMessageToUser", err)
    }

    result, err := state.SendMessageToUser(args.UserName, args.Message)

    if err != nil {
        return nil, errors.NewFunctionReturnedWithError("SendMessageToUser", err)
    }

    return result, nil
}

func SendMessageToUser() {
    var resp *types.Result[string, errors.WeilError]

    result, err := coreSendMessageToUser()

    if err != nil {
        resp = types.NewErrResult[string, errors.WeilError](&err)
    } else {
        resp = types.NewOkResult[string, errors.WeilError](result)
    }

    runtime.SetResult(resp)
}

    func Tools() {
	toolDefs := `[
  {
    "type": "function",
    "function": {
      "name": "list_users",
      "description": "Lists all users visible to the token, including their Slack profile metadata.\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_public_channels",
      "description": "Lists every public channel the token can access, including membership counts.\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_private_channels",
      "description": "Lists private channels (a.k.a. private conversations) the token is a member of.\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_conversations_from_channel",
      "description": "Gets message history from a channel by name, returning a list of messages in the conversation.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "channel_name": {
            "type": "string",
            "description": "the exact channel name to fetch history from\n"
          }
        },
        "required": [
          "channel_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_conversations_with_user",
      "description": "Gets direct message history between the token owner and the specified user.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "user_name": {
            "type": "string",
            "description": "the target user's real name (not display name)\n"
          }
        },
        "required": [
          "user_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "send_message_to_channel",
      "description": "Sends a message to a channel by name.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "channel_name": {
            "type": "string",
            "description": "the exact channel name to post to\n"
          },
          "message": {
            "type": "string",
            "description": "the text content to send\n"
          }
        },
        "required": [
          "channel_name",
          "message"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "send_message_to_user",
      "description": "Sends a direct message to a specific user.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "user_name": {
            "type": "string",
            "description": "the target user's real name (not display name)\n"
          },
          "message": {
            "type": "string",
            "description": "the text content to send\n"
          }
        },
        "required": [
          "user_name",
          "message"
        ]
      }
    }
  }
]`

	runtime.SetResult(types.NewOkResult[string, errors.WeilError](&toolDefs))
}
