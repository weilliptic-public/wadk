package contract
        
import (
)

type SlackConfig struct {
    Token string `json:"token"`
}

type UserProfile struct {
    StatusText string `json:"status_text"`
    StatusEmoji string `json:"status_emoji"`
    Email string `json:"email"`
}

type SlackUser struct {
    Id string `json:"id"`
    Name string `json:"name"`
    RealName string `json:"real_name"`
    Deleted bool `json:"deleted"`
    Tz string `json:"tz"`
    Profile UserProfile `json:"profile"`
}

type PurposeTopic struct {
    Value string `json:"value"`
    Creator string `json:"creator"`
    LastSet uint64 `json:"last_set"`
}

type Channel struct {
    Id string `json:"id"`
    Name string `json:"name"`
    NumMembers uint32 `json:"num_members"`
    IsMember bool `json:"is_member"`
    Purpose PurposeTopic `json:"purpose"`
    Topic PurposeTopic `json:"topic"`
}

type Message struct {
    User string `json:"user"`
    Text string `json:"text"`
}
