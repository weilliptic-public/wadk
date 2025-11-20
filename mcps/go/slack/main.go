package main

import (
	"main/contract"

    "github.com/weilliptic-public/jsonmap"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/errors"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/types"
)

//export __new
func New(len uint, _id uint8) uintptr {
	return runtime.Allocate(len)
}

//export __free
func Free(ptr uintptr, len uint) {
	runtime.Deallocate(ptr, len)
}

//export init
func Init() {
    var resp *types.Result[runtime.WeilValue[contract.SlackContractState, interface{}], errors.WeilError]
	state, err := contract.NewSlackContractState()

    if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("NewSlackContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.SlackContractState, interface{}], errors.WeilError](&newErr)
    } else {
		resp = types.NewOkResult[runtime.WeilValue[contract.SlackContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.SlackContractState, interface{}](state, nil))
    }

	runtime.SetStateAndResult(resp)
}

//export tools
func Tools() {{
	contract.Tools()
}}

//export list_users
func ListUsers() {
    contract.ListUsers()
}

//export list_public_channels
func ListPublicChannels() {
    contract.ListPublicChannels()
}

//export list_private_channels
func ListPrivateChannels() {
    contract.ListPrivateChannels()
}

//export get_conversations_from_channel
func GetConversationsFromChannel() {
    contract.GetConversationsFromChannel()
}

//export get_conversations_with_user
func GetConversationsWithUser() {
    contract.GetConversationsWithUser()
}

//export send_message_to_channel
func SendMessageToChannel() {
    contract.SendMessageToChannel()
}

//export send_message_to_user
func SendMessageToUser() {
    contract.SendMessageToUser()
}

//export method_kind_data
func MethodKindData() {
    methodKindMapping := jsonmap.New()

    methodKindMapping.Set("list_users", "query")
    methodKindMapping.Set("list_public_channels", "query")
    methodKindMapping.Set("list_private_channels", "query")
    methodKindMapping.Set("get_conversations_from_channel", "query")
    methodKindMapping.Set("get_conversations_with_user", "query")
    methodKindMapping.Set("send_message_to_channel", "query")
    methodKindMapping.Set("send_message_to_user", "query")


    resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
