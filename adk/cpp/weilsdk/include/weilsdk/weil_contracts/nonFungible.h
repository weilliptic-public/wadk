#ifndef NON_FUNGIBLE_TOKEN_H
#define NON_FUNGIBLE_TOKEN_H

#include "external/nlohmann.hpp"
#include "weilsdk/collections/map.hpp"
#include "weilsdk/ledger.h"
#include "weilsdk/runtime.h"
#include <set>
#include <variant>
#include <string>
#include <vector>

namespace weilcontracts{

    class Token {
    public:
        // A title for the asset which this NFT represents.
        std::string title;
        // Identifies the asset which this NFT represents.
        std::string name;
        // Describes the asset which this NFT represents
        std::string description;
        // A URI pointing to a resource with mime type image/* representing the asset which this NFT represents.
        std::string payload;
        
        Token(): title(""), name(""), description(""), payload("") {}
        Token(std::string _title, std::string _name, std::string _description, std::string _payload);
    };

    using TokenId = std::string;
    using Address = std::string;

    const TokenId EMPTY_TOKEN_ID = "";
    const Address EMPTY_ADDRESS = "";


    inline void to_json(nlohmann::json& j, const collections::WeilMap<std::string, std::string>& m) {
        j = nlohmann::json::object();
        j["state_id"] = m.getStateId();
    }
    inline void from_json(const nlohmann::json& j, collections::WeilMap<std::string, std::string>& m) {
        m.setStateId(j["state_id"]);
    }

    inline void to_json(nlohmann::json& j, const collections::WeilMap<TokenId, Token>& m) {
        j = nlohmann::json::object();
        j["state_id"] = m.getStateId();
    }
    inline void from_json(const nlohmann::json& j, collections::WeilMap<TokenId, Token>& m) {
        m.setStateId(j["state_id"]);
    }

    inline void to_json(nlohmann::json& j, const collections::WeilMap<Address, std::set<TokenId>>& m) {
        j = nlohmann::json::object();
        j["state_id"] = m.getStateId();
    }
    inline void from_json(const nlohmann::json& j, collections::WeilMap<Address, std::set<TokenId>>& m) {
        m.setStateId(j["state_id"]);
    }


    inline void to_json(nlohmann::json &j, const Token &t) {
        j = nlohmann::json{
            {"title", t.title},
            {"name", t.name},
            {"description", t.description},
            {"payload", t.payload},
        };
    }

    inline void from_json(const nlohmann::json &j, Token &t) {
        j.at("title").get_to(t.title);
        j.at("name").get_to(t.name);
        j.at("description").get_to(t.description);
        j.at("payload").get_to(t.payload);
    }

    class NonFungibleToken {
    private:
        std::string name;
        Address creator;
        collections::WeilMap<TokenId, Token> tokens;
        collections::WeilMap<TokenId, Address> owners;
        collections::WeilMap<Address, std::set<TokenId>> owned;
        collections::WeilMap<std::string, Address> allowances;

        std::pair<bool, std::string> doTransfer(std::string tokenId, Address fromAddr, Address toAddr);

    public:
        NonFungibleToken(std::string _name);
        std::string getName() const;
        void setName(std::string _name);
        std::string getCreator() const;
        bool isValidId(TokenId tokenId);
        bool hasBeenMinted(TokenId tokenId);
        uint32_t balanceOf(std::string addr);
        std::pair<int, Address> ownerOf(TokenId tokenId);
        std::pair<bool, std::variant<Token, std::string>> details(TokenId tokenId);
        std::pair<bool, std::string> transfer(Address toAddr, TokenId tokenId);
        std::pair<bool, std::string> transferFrom(Address fromAddr, Address toAddr, TokenId tokenId);
        std::pair<bool, std::string> approve(Address spender, TokenId tokenId);
        std::pair<bool,std::variant<std::string, std::vector<Address>>> getApproved(TokenId tokenId);
        void setApproveForAll(std::string spender, bool approval);
        bool isApprovedForAll(std::string owner, std::string spender);
        std::pair<int, std::string> mint(TokenId tokenId, Token token);

        collections::WeilMap<TokenId, Token> getTokens() const {return tokens;};
        collections::WeilMap<TokenId, Address> getOwners() const {return owners;};
        collections::WeilMap<Address, std::set<TokenId>> getOwned() const {return owned;};
        collections::WeilMap<std::string, Address> getAllowances() const {return allowances;};

        void setTokens(collections::WeilMap<TokenId, Token> _tokens);
        void setOwners(collections::WeilMap<TokenId, Address> _owners);
        void setOwned( collections::WeilMap<Address, std::set<TokenId>> _owned);
        void setAllowances(collections::WeilMap<std::string, Address> _allowances);
    };

    inline void to_json(nlohmann::json &j, const weilcontracts::NonFungibleToken &tkn) {
        nlohmann::json j1;
        nlohmann::json j2;
        nlohmann::json j3;
        nlohmann::json j4;
        collections::WeilMap<TokenId, Token> token_map = tkn.getTokens();
        collections::WeilMap<TokenId, Address> owners_map = tkn.getOwners();
        collections::WeilMap<Address, std::set<TokenId>> owned_map = tkn.getOwned();
        collections::WeilMap<std::string, Address> allowances_map = tkn.getAllowances();

        to_json(j1,token_map);
        std::string tokens = j1.dump();

        to_json(j2,owners_map);
        std::string owners = j2.dump();

        to_json(j3,owned_map);
        std::string owned = j3.dump();

        to_json(j4,allowances_map);
        std::string allowances = j4.dump();

        j = nlohmann::json{
            {"name", tkn.getName()},
            {"creator", tkn.getCreator()},
            {"tokens", tokens},
            {"owners", owners},
            {"owned", owned},
            {"allowances", allowances}
        };
    }

    inline void from_json(const nlohmann::json &j, weilcontracts::NonFungibleToken &tkn) {
        std::string name = j.at("name").get<std::string>();
        Address creator = j.at("creator").get<Address>();
        
        std::string str1 = j.at("tokens");
        nlohmann::json j1 = nlohmann::json::parse(str1);
        collections::WeilMap<TokenId, Token> tokens;
        from_json(j1,tokens);

        std::string str2 = j.at("owners");
        nlohmann::json j2 = nlohmann::json::parse(str2);
        collections::WeilMap<TokenId, Address> owners;
        from_json(j2,owners);

        std::string str3 = j.at("owned");
        nlohmann::json j3 = nlohmann::json::parse(str3);
        collections::WeilMap<Address, std::set<TokenId>> owned;
        from_json(j3,owned);

        std::string str4 = j.at("allowances");
        nlohmann::json j4 = nlohmann::json::parse(str4);
        collections::WeilMap<std::string, Address> allowances;
        from_json(j4,allowances);

        tkn = NonFungibleToken(name);
        tkn.setTokens(tokens);
        tkn.setOwners(owners);
        tkn.setOwned(owned);
        tkn.setAllowances(allowances);
    }

} // namespace weilcontracts

#endif // NON_FUNGIBLE_TOKEN_H
