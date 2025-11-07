#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/weil_contracts/nonFungible.h"
#include "asciiart.hpp"
#include <string>
#include <map>

extern "C" int __new(size_t len, unsigned char _id)
    __attribute__((export_name("__new")));
extern "C" void __free(size_t ptr, size_t len) __attribute__((export_name("__free")));
extern "C" void init() __attribute__((export_name("init")));
extern "C" void method_kind_data() __attribute__((export_name("method_kind_data")));
extern "C" void is_controller() __attribute__((export_name("is_controller")));
extern "C" void name() __attribute__((export_name("name")));
extern "C" void balance_of() __attribute__((export_name("balance_of")));
extern "C" void owner_of() __attribute__((export_name("owner_of")));
extern "C" void details() __attribute__((export_name("details")));
extern "C" void approve() __attribute__((export_name("approve")));
extern "C" void set_approve_for_all() __attribute__((export_name("set_approve_for_all")));
extern "C" void transfer() __attribute__((export_name("transfer")));
extern "C" void transfer_from() __attribute__((export_name("transfer_from")));
extern "C" void get_approved() __attribute__((export_name("get_approved")));
extern "C" void is_approved_for_all() __attribute__((export_name("is_approved_for_all")));
extern "C" void mint() __attribute__((export_name("mint")));


extern "C" {
    int __new(size_t len, unsigned char _id) {
        void *ptr = weilsdk::Runtime::allocate(len);
        return reinterpret_cast<int>(ptr);
    }

    void __free(size_t ptr, size_t len){
            weilsdk::Runtime::deallocate(ptr, len);
    }


    void method_kind_data() {
        std::map<std::string, std::string> method_kind_mapping;

        method_kind_mapping["name"]= "query";
        method_kind_mapping["balance_of"]= "query";
        method_kind_mapping["is_controller"]= "query";
        method_kind_mapping["owner_of"]= "query";
        method_kind_mapping["details"]= "query";
        method_kind_mapping["approve"]= "mutate";
        method_kind_mapping["set_approve_for_all"]= "mutate";
        method_kind_mapping["transfer"]= "mutate";
        method_kind_mapping["transfer_from"]= "mutate";
        method_kind_mapping["get_approved"]= "query";
        method_kind_mapping["is_approved_for_all"]= "query";
        method_kind_mapping["mint"]= "mutate";

        nlohmann::json json_object = method_kind_mapping;
        std::string serialized_string = json_object.dump();
        weilsdk::Runtime::setResult(serialized_string,0);
    }

    weilcontracts::AsciiArtContractState ascii_instance;

    void init(){
        weilcontracts::AsciiArtContractState ascii_instance("AsciiArt");

        // Add the contract creator as a controller
        std::string creator = weilsdk::Runtime::sender();
        collections::WeilMap<std::string, bool> controllers(0);
        controllers.insert(creator,true);

        // Mint initial tokens
        std::vector<std::pair<std::string, weilcontracts::Token>> initial_tokens = {
            {"0", weilcontracts::Token("A fish going left!", "fish 1", "A one line ASCII drawing of a fish", "<><")},
            {"1", weilcontracts::Token("A fish going right!", "fish 2", "A one line ASCII drawing of a fish swimming to the right", "><>")},
            {"2", weilcontracts::Token("A big fish going left!", "fish 3", "A one line ASCII drawing of a fish swimming to the left", "<'))><")},
            {"3", weilcontracts::Token("A big fish going right!", "fish 4", "A one line ASCII drawing of a fish swimming to the right", "><(('>")},
            {"4", weilcontracts::Token("A Face", "face 1", "A one line ASCII drawing of a face", "(-_-)")},
            {"5", weilcontracts::Token("Arms raised", "arms 1", "A one line ASCII drawing of a person with arms raised", "\\o/")}
        };

        ascii_instance.setControllers(controllers);
        for (const auto& [id, token] : initial_tokens) {
            auto result = ascii_instance.inner.mint(id, token);
            if(result.first){
                std::string error = result.second;
                weilsdk::MethodError me = weilsdk::MethodError("init",error);
                weilsdk::Runtime::setStateAndResult(weilsdk::WeilError::FunctionReturnedWithError(me));
                return;
            }
        }

        nlohmann::json j;
        to_json(j,ascii_instance);
        std::string serialized_state = j.dump();

        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(serialized_state, "null");

        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    struct isControllerArgs{
        std::string addr;
    };

    inline void to_json_0(nlohmann::json &j, const isControllerArgs &k)
    {
        j = nlohmann::json{{"addr", k.addr}};
    }

    inline void from_json_0(const nlohmann::json &j, isControllerArgs &k)
    {
        j.at("addr").get_to(k.addr);
    }

    void is_controller(){

        isControllerArgs args;
        std::string raw_args = weilsdk::Runtime::args();
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("addr"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("is_controller", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        from_json_0(j,args);

        std::string stateString = weilsdk::Runtime::state();
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        bool result = ascii_instance.is_controller(args.addr);

        if(result){
            weilsdk::Runtime::setResult("True", 0);
        }
        else{
            weilsdk::Runtime::setResult("False", 0);
        }
    }

    void name(){
        std::string stateString = weilsdk::Runtime::state();
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        std::string name = ascii_instance.name();
        weilsdk::Runtime::setResult(name,0);   
    }

    struct balanceOfArgs{
        std::string addr;
    };

    inline void to_json_1(nlohmann::json &j, const balanceOfArgs &k)
    {
        j = nlohmann::json{{"addr", k.addr}};
    }

    inline void from_json_1(const nlohmann::json &j, balanceOfArgs &k)
    {
        j.at("addr").get_to(k.addr);
    }

    void balance_of(){
         
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("addr"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("balance_of", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        balanceOfArgs args;
        from_json_1(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        int result = ascii_instance.balance_of(args.addr);
        weilsdk::Runtime::setResult(std::to_string(result),0);
    }

    struct ownerOfArgs{
        std::string token_id;
    };

    inline void to_json_2(nlohmann::json &j, const ownerOfArgs &k)
    {
        j = nlohmann::json{{"token_id", k.token_id}};
    }

    inline void from_json_2(const nlohmann::json &j, ownerOfArgs &k)
    {
        j.at("token_id").get_to(k.token_id);
    }

    void owner_of(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
        
        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("token_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("owner_of", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        
        ownerOfArgs args;
        from_json_2(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        std::string result = ascii_instance.owner_of(args.token_id);
        weilsdk::Runtime::setResult(result,0);
    }

    struct detailsArgs{
        std::string token_id;
    };

    inline void to_json_3(nlohmann::json &j, const detailsArgs &k)
    {
        j = nlohmann::json{{"token_id", k.token_id}};
    }

    inline void from_json_3(const nlohmann::json &j, detailsArgs &k)
    {
        j.at("token_id").get_to(k.token_id);
    }

    void details(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("token_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("details", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 0);
            return;
        }

        detailsArgs args;
        from_json_3(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        std::variant<weilcontracts::TokenDetails, std::string> result = ascii_instance.details(args.token_id);

        if (std::holds_alternative<std::string>(result)) {

            std::string error = std::get<std::string>(result);
            weilsdk::MethodError me = weilsdk::MethodError("details",error);
            weilsdk::Runtime::setResult(weilsdk::WeilError::FunctionReturnedWithError(me),1);
        }
        else if (std::holds_alternative<weilcontracts::TokenDetails>(result)) {
            weilcontracts::TokenDetails token_details = std::get<weilcontracts::TokenDetails>(result);
            nlohmann::ordered_json j1;
            to_json(j1,token_details);

            std::string serialized_result = j1.dump();
            weilsdk::Runtime::setResult(serialized_result,0);
        }
    }

    struct approveArgs{
        std::string spender;
        std::string token_id;
    };

    inline void to_json_4(nlohmann::json &j, const approveArgs &k)
    {
        j = nlohmann::json{{"spender", k.spender},{"token_id", k.token_id}};
    }

    inline void from_json_4(const nlohmann::json &j, approveArgs &k)
    {
        j.at("spender").get_to(k.spender);
        j.at("token_id").get_to(k.token_id);
    }

    void approve(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
       
        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("spender") || !j.contains("token_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("approve", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        approveArgs args;
        from_json_4(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        ascii_instance.approve(args.spender, args.token_id);

        nlohmann::json j2;
        to_json(j2,ascii_instance);

        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(j2.dump(), "null");
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }

    struct approveAllArgs{
        std::string spender;
        bool approval;
    };

    inline void to_json_5(nlohmann::json &j, const approveAllArgs &k)
    {
        j = nlohmann::json{{"spender", k.spender},{"approval", k.approval}};
    }

    inline void from_json_5(const nlohmann::json &j, approveAllArgs &k)
    {
        j.at("spender").get_to(k.spender);
        j.at("approval").get_to(k.approval);
    }

    void set_approve_for_all(){
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("spender") || !j.contains("approval"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("set_approve_for_all", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        approveAllArgs args;
        from_json_5(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        ascii_instance.set_approve_for_all(args.spender, args.approval);

        nlohmann::json j2;
        to_json(j2,ascii_instance);

        weilsdk::WeilValue wv;
        wv.new_with_state_and_ok_value(j2.dump(), "null");
        weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
    }


    struct transferArgs{
        std::string to_addr;
        std::string token_id;
    };

    inline void to_json_6(nlohmann::json &j, const transferArgs &k)
    {
        j = nlohmann::json{{"to_addr", k.to_addr},{"token_id", k.token_id}};
    }

    inline void from_json_6(const nlohmann::json &j, transferArgs &k)
    {
        j.at("to_addr").get_to(k.to_addr);
        j.at("token_id").get_to(k.token_id);
    }

    void transfer(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
        
        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded()  || !j.contains("to_addr") || !j.contains("token_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("transfer", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        
        transferArgs args;
        from_json_6(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        //error, result
        auto result = ascii_instance.transfer(args.to_addr, args.token_id).first;

        if(!result){
            nlohmann::json j2;
            to_json(j2,ascii_instance);
            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(j2.dump(), "null");
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
        }
        else{
            weilsdk::MethodError me = weilsdk::MethodError("transfer","could not transfer");
            std::string err = weilsdk::WeilError::FunctionReturnedWithError(me);
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
        }
    }

    struct transferFromArgs{
        std::string from_addr;
        std::string to_addr;
        std::string token_id;
    };

    inline void to_json_7(nlohmann::json &j, const transferFromArgs &k)
    {
        j = nlohmann::json{{"from_addr", k.from_addr},{"to_addr", k.to_addr},{"token_id", k.token_id}};
    }

    inline void from_json_7(const nlohmann::json &j, transferFromArgs &k)
    {
        j.at("from_addr").get_to(k.from_addr);
        j.at("to_addr").get_to(k.to_addr);
        j.at("token_id").get_to(k.token_id);
    }

    void transfer_from(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("from_addr") || !j.contains("to_addr") || !j.contains("token_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("transfer_from", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        transferFromArgs args;
        from_json_7(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        std::pair<bool, std::string> response = ascii_instance.transfer_from(args.from_addr, args.to_addr, args.token_id);
        bool result = response.first;
        weilsdk::Runtime::debugLog("result of transfer from is "+response.second);
        if(!result){
            nlohmann::json j2;
            to_json(j2,ascii_instance);
            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(j2.dump(), "null");
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
        }
        else{
            weilsdk::MethodError me = weilsdk::MethodError("transfer_from",response.second);
            std::string err = weilsdk::WeilError::FunctionReturnedWithError(me);
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
        }
    }

    struct getApprovedArgs{
        std::string token_id;
    };

    inline void to_json_8(nlohmann::json &j, const getApprovedArgs &k)
    {
        j = nlohmann::json{{"token_id", k.token_id}};
    }

    inline void from_json_8(const nlohmann::json &j, getApprovedArgs &k)
    {
        j.at("token_id").get_to(k.token_id);
    }

    void get_approved(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();
        
        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("token_id"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("get_approved", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        
        getApprovedArgs args;
        from_json_8(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        std::pair<bool, std::variant<std::string, std::vector<std::string>>> approval = ascii_instance.get_approved(args.token_id);

        std::variant<std::string, std::vector<std::string>> result = approval.second;

        if(approval.first){
            std::string error = std::get<std::string>(result);
            weilsdk::MethodError me = weilsdk::MethodError("get_approved",error);
            weilsdk::Runtime::setResult(weilsdk::WeilError::FunctionReturnedWithError(me),1);
        }
        else{
            std::vector<std::string> approved = std::get< std::vector<std::string>>(result);
            nlohmann::json j2= approved;
            weilsdk::Runtime::setResult(j2.dump(),0);
        }
    }

    struct isApprovedAllArgs{
        std::string owner;
        std::string spender;
    };

    inline void to_json_9(nlohmann::json &j, const isApprovedAllArgs &k)
    {
        j = nlohmann::json{{"owner", k.owner}, {"spender", k.spender}};
    }

    inline void from_json_9(const nlohmann::json &j, isApprovedAllArgs &k)
    {
        j.at("owner").get_to(k.owner);
        j.at("spender").get_to(k.spender);
    }

    void is_approved_for_all(){

        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("owner") || !j.contains("spender"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("is_approved_for_all", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }

        isApprovedAllArgs args;
        from_json_9(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);

        bool result = ascii_instance.is_approved_for_all(args.owner, args.spender);
        weilsdk::Runtime::setResult(std::to_string(result),0);
    }

    struct mintArgs{
        std::string token_id;
        std::string title;
        std::string name;
        std::string description;
        std::string payload;
    };

    inline void to_json_10(nlohmann::json &j, const mintArgs &k)
    {
        j = nlohmann::json{{"token_id", k.token_id}, {"title", k.title}, {"name", k.name}, {"description", k.description},{"payload", k.payload}};
    }

    inline void from_json_10(const nlohmann::json &j, mintArgs &k)
    {
        j.at("token_id").get_to(k.token_id);
        j.at("title").get_to(k.title);
        j.at("name").get_to(k.name);
        j.at("description").get_to(k.description);
        j.at("payload").get_to(k.payload);
    }

    void mint(){
        std::pair<std::string, std::string> p = weilsdk::Runtime::stateAndArgs();

        std::string raw_args = p.second;
        nlohmann::json j = nlohmann::json::parse(raw_args);

        if (j.is_discarded() || !j.contains("token_id") || !j.contains("title") || !j.contains("name") || !j.contains("description") || !j.contains("payload"))
        {
            weilsdk::MethodError me = weilsdk::MethodError("mint", "invalid_args");
            weilsdk::Runtime::setResult(weilsdk::WeilError::MethodArgumentDeserializationError(me), 1);
            return;
        }
        mintArgs args;
        from_json_10(j,args);

        std::string stateString = p.first;
        nlohmann::json j1 = nlohmann::json::parse(stateString);

        from_json(j1,ascii_instance);
        std::pair<int,std::string> result = ascii_instance.mint(args.token_id, args.title, args.name, args.description, args.payload);

        nlohmann::json j2;
        to_json(j2,ascii_instance);

        if(result.first){
            std::string error = result.second;
            weilsdk::MethodError me = weilsdk::MethodError("mint",error);
            std::string err = weilsdk::WeilError::FunctionReturnedWithError(me);
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {err});
        }
        else{
            weilsdk::WeilValue wv;
            wv.new_with_state_and_ok_value(j2.dump(), "null");
            weilsdk::Runtime::setStateAndResult(std::variant<weilsdk::WeilValue,std::string> {wv});
        }

    }
}



