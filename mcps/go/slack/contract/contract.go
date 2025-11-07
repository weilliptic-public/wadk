package contract

import (
	"encoding/json"
	"errors"
	config "github.com/weilliptic-public/wadk/adk/go/weil_go/config"
)


// SlackContractState holds Slack configuration secrets and request helpers.
type SlackContractState struct {
	Secrets config.Secrets[SlackConfig]
}

// NewSlackContractState constructs a SlackContractState with initialized secrets storage.
func NewSlackContractState() (*SlackContractState, error) {
	return &SlackContractState{
		Secrets: config.NewSecrets[SlackConfig](),
	}, nil
}

// ListUsers retrieves Slack users visible to the configured token.
func (c *SlackContractState) ListUsers() (*[]SlackUser, error) {
	path := "users.list"
	query_params := make(map[string]string)

	response, err := c.sendRequest("GET", path, query_params, nil)

	if err != nil {
		return nil, err
	}

	var UserList SlackUserListResponse

	if err := json.Unmarshal(response, &UserList); err != nil {
		return nil, err
	}

	return &UserList.Members, nil
}

// ListPublicChannels fetches public channels the token can access.
func (c *SlackContractState) ListPublicChannels() (*[]Channel, error) {
	return c.ListChannels("public_channel")
}

// GetConversationsFromChannel grabs message history for the provided channel name.
func (c *SlackContractState) GetConversationsFromChannel(channelName string) (*[]Message, error) {
	channelId, err := c.FindChannelId(channelName)
	if err != nil {
		return nil, err
	}

	return c.GetConversations(*channelId)
}

// GetConversationsWithUser retrieves direct message history with the specified user name.
func (obj *SlackContractState) GetConversationsWithUser(userName string) (*[]Message, error) {
	// two steps : get the conversation id pertaining to the user Direct Message, AND then : get the messages for this conversation id

	// STEP 1
	path := "users.conversations"

	query_params := make(map[string]string)
	query_params["types"] = "im"

	response, err := obj.sendRequest("GET", path, query_params, nil)
	if err != nil {
		return nil, err
	}

	var Conversations UserConversationsResponse
	if err := json.Unmarshal(response, &Conversations); err != nil {
		return nil, err
	}

	// STEP 2
	userId, err := obj.GetUserId(userName)
	if err != nil {
		return nil, err
	}

	for _, userConversation := range Conversations.UserPtrs {
		if userConversation.User == userId {
			return obj.GetConversations(userConversation.Id)
		}
	}

	return nil, errors.New("user not found")
}

// SendMessageToChannel publishes a message to the given channel name.
func (obj *SlackContractState) SendMessageToChannel(channelName string, message string) (*string, error) {
	err := obj.SendMessage(channelName, message)
	if err != nil {
		return nil, err
	}

	successMsg := "message sent successfully"
	return &successMsg, nil
}

// query
// SendMessageToUser publishes a direct message to the given user name.
func (obj *SlackContractState) SendMessageToUser(userName string, message string) (*string, error) {
	userId, GetUserErr := obj.GetUserId(userName)

	if GetUserErr != nil {
		return nil, GetUserErr
	}

	err := obj.SendMessage(userId, message)
	if err != nil {
		return nil, err
	}

	successMsg := "message sent successfully"
	return &successMsg, nil

}

// ListPrivateChannels fetches private channels the token is a member of.
func (c *SlackContractState) ListPrivateChannels() (*[]Channel, error) {
	return c.ListChannels("private_channel")
}
