#include "weilsdk/weil_contracts/nonFungible.h"
#include <variant>
#include <vector>
#include <string>

namespace weilcontracts {

    Token::Token(std::string _title, std::string _name, std::string _description,
                std::string _payload) {
        this->title = _title;
        this->description = _description;
        this->name = _name;
        this->payload = _payload;
    }

    NonFungibleToken::NonFungibleToken(std::string _name) {
        this->name = _name;
        this->creator = weilsdk::Runtime::sender();
        this->tokens = collections::WeilMap<TokenId, Token>(1);
        this->owners = collections::WeilMap<TokenId, Address>(2);
        this->owned = collections::WeilMap<Address, std::set<TokenId>>(3);
        this->allowances = collections::WeilMap<std::string, Address>(4);
    }

    std::string NonFungibleToken::getName() const { return name; }

    void NonFungibleToken::setName(std::string _name) { name = _name;}

    std::string NonFungibleToken::getCreator() const { return creator; }

    void NonFungibleToken::setTokens(collections::WeilMap<TokenId, Token> _tokens) {
        this->tokens = _tokens;
    }

    void NonFungibleToken::setOwners(
        collections::WeilMap<TokenId, Address> _owners) {
        this->owners = _owners;
    }

    void NonFungibleToken::setOwned(
        collections::WeilMap<Address, std::set<TokenId>> _owned) {
        this->owned = _owned;
    }

    void NonFungibleToken::setAllowances(
        collections::WeilMap<std::string, Address> _allowances) {
        this->allowances = _allowances;
    }

    bool NonFungibleToken::isValidId(TokenId tokenId) {
        return tokenId.length() > 0 && tokenId.length() < 256;
    }

    bool NonFungibleToken::hasBeenMinted(TokenId tokenId) {
        collections::WeilMap<std::string, std::string> mp(this->owners.getStateId());
        bool c1 = mp.contains(tokenId);
        bool c2 = mp.get(tokenId) != EMPTY_TOKEN_ID;

        return c1 && c2;
    }

    uint32_t NonFungibleToken::balanceOf(std::string addr) {
        return this->getOwned().contains(addr) ? this->getOwned().get(addr).size()
                                            : 0;
    }

    std::pair<int, Address> NonFungibleToken::ownerOf(TokenId tokenId) {
        if (!isValidId(tokenId)) {
            return {1, tokenId + " is not a valid id"};
        }
        if (!this->getOwners().contains(tokenId)) {
            return {1, "Owner of " + tokenId + " is not identified"};
        }
        collections::WeilMap<std::string, std::string> mp(this->owners.getStateId());
        return {0, "\"" + mp.get(tokenId) + "\""};
    }

    std::pair<bool, std::variant<Token, std::string>>
    NonFungibleToken::details(TokenId tokenId) {

        if (!isValidId(tokenId)) {
            return {1, tokenId + " is not a valid id"};
        }

        if (!hasBeenMinted(tokenId)) {
            return {1, tokenId + " has not been minted yet"};
        }
        collections::WeilMap<TokenId, Token> tokens_map(
            this->getTokens().getStateId());

        if (!tokens_map.contains(tokenId)) {
            return {1, "token " + tokenId + " not found"};
        }
        return {0, tokens_map.get(tokenId)};
    }

    std::pair<bool, std::string> NonFungibleToken::doTransfer(std::string tokenId,
                                                            Address fromAddr,
                                                            Address toAddr) {
        bool result = weilsdk::Ledger::transfer(tokenId, fromAddr, toAddr, 1).first;

        if (!result) {
            return {1, tokenId + " could not be transferred by the Ledger"};
        }

        collections::WeilMap<std::string,std::string> ownersMap(this->getOwners().getStateId());
        ownersMap.insert(tokenId, toAddr);
        collections::WeilMap<std::string, std::set<TokenId>> ownedMap(this->getOwned().getStateId());
        std::set<TokenId> oldOwnersTokens = ownedMap.get(fromAddr);
        if (oldOwnersTokens.empty()) {
            return {1, "Owned tokens is missing"};
        }

        oldOwnersTokens.erase(tokenId);
        ownedMap.insert(fromAddr, oldOwnersTokens);

        std::set<TokenId> newOwnersTokens = ownedMap.get(toAddr);
        newOwnersTokens.insert(tokenId);
        ownedMap.insert(toAddr, newOwnersTokens);

        std::string key = fromAddr + "$" + tokenId;
        collections::WeilMap<std::string, std::string> allowancesMap(this->getAllowances().getStateId());
        allowancesMap.remove(key);

        return {0, "Ok"};
    }

    std::pair<bool, std::string> NonFungibleToken::transfer(Address toAddr,
                                                            TokenId tokenId) {
        Address fromAddr = weilsdk::Runtime::sender();

        collections::WeilMap<std::string, std::string> ownersMap(this->getOwners().getStateId());                                                

        if (!isValidId(tokenId)) {
            return {1, "Token " + tokenId + " is not a valid token id"};
        }

        if (!ownersMap.contains(tokenId)) {
            return {1, "Token " + tokenId + " is missing an owner"};
        }
        Address oldOwner = ownersMap.get(tokenId);
        if (oldOwner != fromAddr) {
            return {1, "Token " + tokenId + " not owned by " + fromAddr};
        }

        return doTransfer(tokenId, fromAddr, toAddr);
    }

    std::pair<bool, std::string> NonFungibleToken::transferFrom(Address fromAddr,
                                                                Address toAddr,
                                                                TokenId tokenId) {
        Address spender = weilsdk::Runtime::sender();

        if (!isValidId(tokenId)) {
            return {1, "token " + tokenId + " is not a valid token id"};
        }

        if (!this->getOwners().contains(tokenId)) {
            return {1, "token " + tokenId + " is missing an owner"};
        }

        Address oldOwner = this->getOwners().get(tokenId);
        if (oldOwner != fromAddr) {
            return {1, "token " + tokenId + " not owned by " + fromAddr};
        }

        std::string key = oldOwner + "$" + tokenId;
        bool allowed = this->getAllowances().contains(key) &&
                        this->getAllowances().get(key) == spender;

        if (!allowed) {
            std::string key2 = oldOwner + "$" + EMPTY_TOKEN_ID;
            if (!this->getAllowances().contains(key2) || this->getAllowances().get(key2) != spender){
                return {1, "transfer of token `" + tokenId + "` not authorized"};
            }
        }

        return doTransfer(tokenId, fromAddr, toAddr);
    }

    std::pair<bool, std::string> NonFungibleToken::approve(Address spender,
                                                        TokenId tokenId) {
        Address fromAddr = weilsdk::Runtime::sender();

        if (!isValidId(tokenId)) {
            return {1, "token `" + tokenId + " is not a valid token id"};
        }

        if (!this->getOwners().contains(tokenId)) {
            return {1, "token `" + tokenId + "` is missing an owner"};
        }

        Address oldOwner = this->getOwners().get(tokenId);
        if (oldOwner != fromAddr) {
            return {1, "token `" + tokenId + "` not owned by " + fromAddr};
        }

        std::string key = oldOwner + "$" + tokenId;
        if (spender == EMPTY_ADDRESS) {
            this->getAllowances().remove(key);
        } else {
            this->getAllowances().insert(key, spender);
        }

        return {0, "Ok"};
    }

    std::pair<bool, std::variant<std::string, std::vector<Address>>>
    NonFungibleToken::getApproved(TokenId tokenId) {
        std::vector<Address> response;

        if (!isValidId(tokenId)) {
            return {1, "token `" + tokenId + "` is not a valid token id"};
        }

        if (!this->getOwners().contains(tokenId)) {
            return {1, "token `" + tokenId + "` is missing an owner"};
        }

        std::string owner = this->getOwners().get(tokenId);

        std::string key1 = owner + "$" + tokenId;
        if (this->getAllowances().contains(key1)) {
            response.push_back(this->getAllowances().get(key1));
        }

        std::string key2 = owner + "$" + EMPTY_TOKEN_ID;
        if (this->getAllowances().contains(key2)) {
            response.push_back(this->getAllowances().get(key2));
        }

        return {0, response};
    }

    void NonFungibleToken::setApproveForAll(std::string spender, bool approval) {
    Address fromAddr = weilsdk::Runtime::sender();
    std::string key = fromAddr + "$" + EMPTY_TOKEN_ID;

        if (approval) {
            this->getAllowances().insert(key, spender);
        } else {
            this->getAllowances().remove(key);
        }
    }

    bool NonFungibleToken::isApprovedForAll(std::string owner,
                                            std::string spender) {
        std::string key = owner + "$" + EMPTY_TOKEN_ID;
        return this->getAllowances().contains(key) &&
               this->getAllowances().get(key) == spender;
    }

    std::pair<int, std::string> NonFungibleToken::mint(TokenId tokenId,
                                                    Token token) {
        Address sender = weilsdk::Runtime::sender();

        if (!isValidId(tokenId)) {
            return {1, "invalid token id"};
        }
        if(this->getTokens().contains(tokenId)){
            return {1, "token id `" + tokenId + "` already minted " +
                tokens.get(tokenId).name};
        }

        auto res = weilsdk::Ledger::mint(tokenId, sender, 1);
        if(!res.first){
            return {1,"could not mint through ledger"};
        }

        collections::WeilMap<TokenId, Token> tokenMap(this->getTokens().getStateId());
        collections::WeilMap<std::string, std::string> ownersMap(this->getOwners().getStateId());
        collections::WeilMap<Address, std::set<TokenId>> ownedMap(this->getOwned().getStateId());
        tokenMap.insert(tokenId, token);
        ownersMap.insert(tokenId, sender);

        if (!ownedMap.contains(sender)) {
            ownedMap.insert(sender, std::set<TokenId>());
        }

        std::set<TokenId> senderTokens = ownedMap.get(sender);
        senderTokens.insert(tokenId);
        ownedMap.insert(sender, senderTokens);
        return {0, tokenId + " has been minted by " + sender};
    }

} // namespace weilcontracts
