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
    var resp *types.Result[runtime.WeilValue[contract.FirstContractState, interface{}], errors.WeilError]
	state, err := contract.NewFirstContractState()

    if err != nil {
		var newErr errors.WeilError = errors.NewFunctionReturnedWithError("NewFirstContractState", err)
		resp = types.NewErrResult[runtime.WeilValue[contract.FirstContractState, interface{}], errors.WeilError](&newErr)
    } else {
		resp = types.NewOkResult[runtime.WeilValue[contract.FirstContractState, interface{}], errors.WeilError](runtime.NewWeilValueWithStateAndOkValue[contract.FirstContractState, interface{}](state, nil))
    }

	runtime.SetStateAndResult(resp)
}

//export health_check
func HealthCheck() {
    contract.HealthCheck()
}

//export counter
func Counter() {
    contract.Counter()
}

//export set_list_in_second
func SetListInSecond() {
    contract.SetListInSecond()
}

//export set_list_in_second_callback
func SetListInSecondCallback() {
    contract.SetListInSecondCallback()
}

//export method_kind_data
func MethodKindData() {
    methodKindMapping := jsonmap.New()

    methodKindMapping.Set("health_check", "query")
    methodKindMapping.Set("counter", "query")
    methodKindMapping.Set("set_list_in_second", "mutate")
    methodKindMapping.Set("set_list_in_second_callback", "mutate")


    resp := types.NewOkResult[jsonmap.Map, errors.WeilError](methodKindMapping)
	runtime.SetResult(resp)
}

func main() {}
