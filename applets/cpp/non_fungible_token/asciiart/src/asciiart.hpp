#include "weilsdk/error.h"
#include "weilsdk/runtime.h"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/weil_contracts/nonFungible.h"

namespace weilcontracts
{

    struct TokenDetails{
        std::string title;
        std::string name;
        std::string description;
        std::string payload;
    };

    inline void to_json(nlohmann::ordered_json &j, const TokenDetails &td) {
        j = nlohmann::ordered_json{
            {"title", td.title},
            {"name", td.name},
            {"description", td.description},
            {"payload", td.payload}
        };
    }

    inline void from_json(const nlohmann::ordered_json &j, TokenDetails &td) {
        j.at("title").get_to(td.title);
        j.at("name").get_to(td.name);
        j.at("description").get_to(td.description);
        j.at("payload").get_to(td.payload);
    }


    class AsciiArtContractState {
    public:
        collections::WeilMap<std::string, bool> controllers;
        NonFungibleToken inner;

        AsciiArtContractState() : inner("") {}
        AsciiArtContractState(std::string _name) : inner(_name) {}

        bool is_controller(const std::string& addr) {
            if(!this->getControllers().contains(addr)){
                return false;
            }
            else return this->getControllers().get(addr);
        }

        std::string name() const {
            return inner.getName();
        }

        size_t balance_of(const std::string& addr) {
            return inner.balanceOf(addr);
        }

        std::string owner_of(const std::string& token_id) {
            return inner.ownerOf(token_id).second;
        }

        collections::WeilMap<std::string, bool> getControllers() const{
            return controllers;
        }

        void setControllers(collections::WeilMap<std::string, bool> _controllers) {
            controllers = _controllers;
        }

        const NonFungibleToken& getInner() const{
            return inner;
        }

        void setInner(const NonFungibleToken& _inner) {inner = _inner;}

    /*
    The function details may either return a string(indicating an error) or TokenDetails

    Here's how it is intended to be consumed:

        std::variant<TokenDetails, std::string> result = details(token_id);
        if (std::holds_alternative<std::string>(result)) {
            // Extract and handle the error message
            std::string error = std::get<std::string>(result);
        }
        // Check if the result contains TokenDetails
        else if (std::holds_alternative<TokenDetails>(result)) {
            // Extract and handle the token details
            TokenDetails token_details = std::get<TokenDetails>(result);
        }

    */
        std::variant<TokenDetails, std::string> details(const std::string& tokenId) {
            auto result =  inner.details(tokenId);
            if(result.first){
                //there was some error
                return std::get<std::string>(result.second);
            }
            weilcontracts::Token token = std::get<weilcontracts::Token>(result.second);
            TokenDetails tokenDetails {
                token.title,
                token.name,
                token.description,
                token.payload
            };
            return tokenDetails;
        }

        void approve(const std::string& spender, const std::string& tokenId) {
            inner.approve(spender, tokenId);
        }

        void set_approve_for_all(const std::string& spender, bool approval) {
            inner.setApproveForAll(spender, approval);
        }

        std::pair<bool, std::string>  transfer(const std::string& toAddr, const std::string& tokenId) {
            return inner.transfer(toAddr, tokenId);
        }

        std::pair<bool, std::string> transfer_from(const std::string& fromAddr, const std::string& toAddr, const std::string& tokenId) {
            return inner.transferFrom(fromAddr, toAddr, tokenId);
        }

        std::pair<bool, std::variant<std::string, std::vector<Address>>> get_approved(const TokenId tokenId){
            return inner.getApproved(tokenId);
        }

        bool is_approved_for_all(std::string owner, std::string spender){
            return inner.isApprovedForAll(owner, spender);
        }

        //returns {error, corresponding string}
        std::pair<int,std::string> mint(const std::string& tokenId, const std::string& title, const std::string& name, const std::string& description, const std::string& payload) {
            
            std::string fromAddr = weilsdk::Runtime::sender();

            if (!is_controller(fromAddr)) {
                return {1, "Only controllers can mint"};
            }

            weilcontracts::Token token(title, name, description, payload);
            return inner.mint(tokenId, token);
        }

    };

    inline void to_json(nlohmann::json& j, const collections::WeilMap<std::string, bool> &m) {
        j = nlohmann::json::object();
        j["state_id"] = m.getStateId();
    }

    inline void from_json(const nlohmann::json& j, collections::WeilMap<std::string, bool> &m) {
        m.setStateId(j["state_id"]);
    }

    inline void to_json(nlohmann::json &j, const AsciiArtContractState &asciiart){
        nlohmann::json j1;
        to_json(j1, asciiart.getControllers());

        std::string serialized_controllers_state = j1.dump();

        j = nlohmann::json{
            {"name", asciiart.getInner().getName()},
            {"controllers", serialized_controllers_state}
        };
    }


    inline void from_json(const nlohmann::json &j, AsciiArtContractState &asciiart) {

        std::string serialized_controllers = j.at("controllers");
        nlohmann::json j1  = nlohmann::json::parse(serialized_controllers);
        collections::WeilMap<std::string, bool> mp;
        from_json(j1,mp);
        asciiart.setControllers(mp);

        std::string name = j.at("name");
        NonFungibleToken nft(name);

        asciiart.setInner(nft);
    }

}