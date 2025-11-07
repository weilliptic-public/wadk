package contract

import (
	"encoding/json"
	"errors"
	"strings"
	http "github.com/weilliptic-public/wadk/adk/go/weil_go/http"
)

func (obj *SlackContractState) sendRequest(method string, path string, queryParams map[string]string, body *[]byte) ([]byte, error) {
	url := "https://slack.com/api/" + path

	client := http.NewHttpClient()
	var httpMethod http.HttpMethod
	switch strings.ToUpper(method) {
	case "GET":
		httpMethod = http.GET
	case "POST":
		httpMethod = http.POST
	case "PUT":
		httpMethod = http.PUT
	case "DELETE":
		httpMethod = http.DELETE
	case "PATCH":
		httpMethod = http.PATCH
	default:
		return nil, errors.New("unsupported HTTP method: " + method)
	}

	header := make(map[string]string)
	config, err := obj.Secrets.Config()
	if err != nil {
		return []byte{}, nil
	}

	header["Authorization"] = "Bearer " + config.Token
	req := client.Request(httpMethod, url).Headers(header).Query(queryParams)
	if body != nil {
		bodyStr := string(*body)
		req = req.Body(bodyStr)
	}
	response := req.Send()

	if response.IsErrResult() {
		return []byte{}, *response.TryErrResult()
	}
	return []byte(response.TryOkResult().Text()), nil
}

// TODO: Join event still might be getting in
// GetConversations returns message history for a channel or direct message by Slack conversation ID.
func (c *SlackContractState) GetConversations(ConversationId string) (*[]Message, error) {
	path := "conversations.history"
	query_params := make(map[string]string)

	query_params["channel"] = *&ConversationId
	response, err := c.sendRequest("GET", path, query_params, nil)
	if err != nil {
		return nil, err
	}
	var Conversations SlackConversationHistoryResponse

	if err := json.Unmarshal(response, &Conversations); err != nil {
		return nil, err
	}

	messages := make([]Message, 0)

	userMap, err := c.GetUserIdToUserMapping()
	if err != nil {
		return nil, err
	}
	for _, event := range Conversations.Messages {
		if event.SubType != nil {
			continue
		}
		messages = append(messages, Message{User: userMap[event.UserId], Text: event.Text})
	}

	return &messages, nil
}

// GetUserId returns the Slack user ID matching the provided real name.
func (c *SlackContractState) GetUserId(UserName string) (string, error) {
	listUsers, err := c.ListUsers()
	if err != nil {
		return "", errors.New("List Users returned error")
	}

	for _, user := range *listUsers {
		if strings.ToLower(user.RealName) == strings.ToLower(UserName) {
			return user.Id, nil
		}
	}

	return "", errors.New("User not found")
}

// GetUserIdToUserMapping maps user IDs to real names for quick lookup.
func (c *SlackContractState) GetUserIdToUserMapping() (map[string]string, error) {
	listUsers, err := c.ListUsers()
	if err != nil {
		return nil, err
	}

	userIdToUser := make(map[string]string)
	for _, user := range *listUsers {
		userIdToUser[user.Id] = user.RealName
	}

	return userIdToUser, nil
}

// FindChannelId resolves a channel name to its Slack channel ID across public and private channels.
func (c *SlackContractState) FindChannelId(ChannelName string) (*string, error) {
	ChannelList, err := c.ListChannels("public_channel, private_channel")
	if err != nil {
		return nil, err
	}
	for _, channel := range *ChannelList {
		if channel.Name == ChannelName {
			return &channel.Id, nil
		}
	}
	return nil, errors.New("Channel not found")
}

// ListChannels retrieves channels of the provided Slack types value.
func (c *SlackContractState) ListChannels(ChannelType string) (*[]Channel, error) {
	path := "conversations.list"

	query_params := make(map[string]string)
	query_params["types"] = ChannelType

	response, err := c.sendRequest("GET", path, query_params, nil)

	if err != nil {
		return nil, err
	}
	var channelList SlackChannelListResponse
	if err := json.Unmarshal(response, &channelList); err != nil {
		return nil, err
	}

	return &channelList.Channels, nil
}

// TODO: emoji
// SendMessage posts a message to the given conversation ID.
func (c *SlackContractState) SendMessage(EntityId string, Message string) error {
	path := "chat.postMessage"

	query_params := make(map[string]string)

	msgReq := PostMessageRequest{
		Text:     Message,
		EntityId: EntityId,
		AsUser:   true,
	}

	var body, err = json.Marshal(msgReq)

	if err != nil {
		return err
	}

	response, err := c.sendRequest("POST", path, query_params, &body)

	if err != nil {
		return err
	}

	var msgResponse MessageResponse
	json.Unmarshal(response, &msgResponse)

	if msgResponse.Error != nil {
		return errors.New(*msgResponse.Error)
	}

	return nil
}
