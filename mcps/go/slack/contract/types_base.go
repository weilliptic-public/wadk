package contract

import ()

// SlackUserListResponse models the users.list Slack response.
type SlackUserListResponse struct {
	Ok       bool             `json:"ok"`
	Members  []SlackUser      `json:"members"`
	CacheTS  int64            `json:"cache_ts"`
	Metadata ResponseMetadata `json:"response_metadata"`
}

// MessageResponse captures the Slack chat.postMessage acknowledgement.
type MessageResponse struct {
	Ok    string
	Error *string `json:"errror,omitempty"`
}

// PostMessageRequest represents the payload to Slack's chat.postMessage.
type PostMessageRequest struct {
	EntityId string `json:"channel"`
	Text     string `json:"text"`
	AsUser   bool   `json:"as_user"`
}

// ListScheduleMessagesRequest describes parameters for scheduled messages.
type ListScheduleMessagesRequest struct {
	Channel string
	Limit   int
}

// SlackChannelListResponse models the conversations.list response.
type SlackChannelListResponse struct {
	Ok       bool             `json:"ok"`
	Channels []Channel        `json:"channels"`
	Error    string           `json:"error,omitempty"`
	Metadata ResponseMetadata `json:"response_metadata"`
}

// Purpose mirrors Slack channel purpose fields.
type Purpose struct {
	Value   string `json:"value"`
	Creator string `json:"creator"`
	LastSet int64  `json:"last_set"`
}

// Topic mirrors Slack channel topic fields.
type Topic struct {
	Value   string `json:"value"`
	Creator string `json:"creator"`
	LastSet int64  `json:"last_set"`
}

// ResponseMetadata includes Slack pagination state.
type ResponseMetadata struct {
	NextCursor string `json:"next_cursor"`
}

// SlackConversationHistoryResponse models conversation history results.
type SlackConversationHistoryResponse struct {
	Ok       bool                `json:"ok"`
	Messages []ConversationEvent `json:"messages,omitempty"`
	Error    string              `json:"error,omitempty"`
	Metadata ResponseMetadata    `json:"response_metadata"`
}

// ConversationEvent captures a single Slack event in channel history.
type ConversationEvent struct {
	UserId  string  `json:"user"`
	Text    string  `json:"text"`
	SubType *string `json:"subtype,omitempty"`
}

// UserConversationsResponse models users.conversations results.
type UserConversationsResponse struct {
	Ok       bool                      `json:"ok"`
	Error    string                    `json:"error,omitempty"`
	Metadata ResponseMetadata          `json:"response_metadata"`
	UserPtrs []UserCoversationPointers `json:"channels"`
}

// UserCoversationPointers points to a user's direct message channel.
type UserCoversationPointers struct {
	Id          string `json:"id"`
	User        string `json:"user"`
	DeletedUser bool   `json:"is_user_deleted"`
}
