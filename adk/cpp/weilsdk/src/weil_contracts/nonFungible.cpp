/**
 * @file nonFungible.cpp
 * @brief Implementation of non-fungible token (NFT) contract operations
 * @details This file provides implementations for ERC-721-like non-fungible token operations
 *          including token minting, transfers, approvals, ownership queries, and token details.
 */

#include "weilsdk/weil_contracts/nonFungible.h"
#include <variant>
#include <vector>
#include <string>

namespace weilcontracts {

    /**
     * @brief Constructs a Token with the given metadata
     * @param _title The title of the token
     * @param _name The name of the token
     * @param _description The description of the token
     * @param _payload Additional payload data for the token
     */
    Token::Token(std::string _title, std::string _name, std::string _description,
                std::string _payload) {
        this->title = _title;
        this->description = _description;
        this->name = _name;
        this->payload = _payload;
    }

    /**
     * @brief Constructs a NonFungibleToken with the given name
     * @param _name The name of the NFT collection
     */
    NonFungibleToken::NonFungibleToken(std::string _name) {
        this->name = _name;
        this->creator = weilsdk::Runtime::sender();
        this->tokens = collections::WeilMap<TokenId, Token>(1);
        this->owners = collections::WeilMap<TokenId, Address>(2);
        this->owned = collections::WeilMap<Address, std::set<TokenId>>(3);
        this->allowances = collections::WeilMap<std::string, Address>(4);
    }

    /**
     * @brief Gets the name of the NFT collection
     * @return The collection name
     */
    std::string NonFungibleToken::getName() const { return name; }

    /**
     * @brief Sets the name of the NFT collection
     * @param _name The new name to set
     */
    void NonFungibleToken::setName(std::string _name) { name = _name;}

    /**
     * @brief Gets the creator address of the NFT collection
     * @return The creator address
     */
    std::string NonFungibleToken::getCreator() const { return creator; }

    /**
     * @brief Sets the tokens map for the NFT collection
     * @param _tokens The new tokens map to set
     */
    void NonFungibleToken::setTokens(collections::WeilMap<TokenId, Token> _tokens) {
        this->tokens = _tokens;
    }

    /**
     * @brief Sets the owners map for the NFT collection
     * @param _owners The new owners map to set
     */
    void NonFungibleToken::setOwners(
        collections::WeilMap<TokenId, Address> _owners) {
        this->owners = _owners;
    }

    /**
     * @brief Sets the owned map for the NFT collection
     * @param _owned The new owned map to set
     */
    void NonFungibleToken::setOwned(
        collections::WeilMap<Address, std::set<TokenId>> _owned) {
        this->owned = _owned;
    }

    /**
     * @brief Sets the allowances map for the NFT collection
     * @param _allowances The new allowances map to set
     */
    void NonFungibleToken::setAllowances(
        collections::WeilMap<std::string, Address> _allowances) {
        this->allowances = _allowances;
    }

    /**
     * @brief Validates if a token ID is valid
     * @param tokenId The token ID to validate
     * @return true if the token ID is valid (length > 0 and < 256), false otherwise
     */
    bool NonFungibleToken::isValidId(TokenId tokenId) {
        return tokenId.length() > 0 && tokenId.length() < 256;
    }

    /**
     * @brief Checks if a token has been minted
     * @param tokenId The token ID to check
     * @return true if the token has been minted, false otherwise
     */
    bool NonFungibleToken::hasBeenMinted(TokenId tokenId) {
        collections::WeilMap<std::string, std::string> mp(this->owners.getStateId());
        bool c1 = mp.contains(tokenId);
        bool c2 = mp.get(tokenId) != EMPTY_TOKEN_ID;

        return c1 && c2;
    }

    /**
     * @brief Gets the balance (number of tokens owned) for a given address
     * @param addr The address to query the balance for
     * @return The number of tokens owned by the address
     */
    uint32_t NonFungibleToken::balanceOf(std::string addr) {
        return this->getOwned().contains(addr) ? this->getOwned().get(addr).size()
                                            : 0;
    }

    /**
     * @brief Gets the owner address of a specific token
     * @param tokenId The token ID to query
     * @return A pair where the first element indicates success (0) or failure (1),
     *         and the second element contains the owner address or error message
     */
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

    /**
     * @brief Gets the details of a specific token
     * @param tokenId The token ID to query
     * @return A pair where the first element indicates success (false) or failure (true),
     *         and the second element contains the Token object or error message
     */
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

    /**
     * @brief Internal function to perform a token transfer
     * @details This function handles the actual transfer logic including updating
     *          ownership records and allowance mappings.
     * @param tokenId The token ID to transfer
     * @param fromAddr The address to transfer from
     * @param toAddr The address to transfer to
     * @return A pair where the first element indicates success (false) or failure (true),
     *         and the second element contains the result message
     */
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

    /**
     * @brief Transfers a token from the sender to the specified address
     * @param toAddr The address to transfer the token to
     * @param tokenId The token ID to transfer
     * @return A pair where the first element indicates success (false) or failure (true),
     *         and the second element contains the result message
     */
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

    /**
     * @brief Transfers a token from one address to another on behalf of the sender
     * @details This function allows a spender to transfer a token if they have been
     *          approved by the owner.
     * @param fromAddr The address to transfer the token from
     * @param toAddr The address to transfer the token to
     * @param tokenId The token ID to transfer
     * @return A pair where the first element indicates success (false) or failure (true),
     *         and the second element contains the result message
     */
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

    /**
     * @brief Approves a spender to transfer a specific token on behalf of the sender
     * @param spender The address that will be approved to transfer the token
     * @param tokenId The token ID to approve for transfer
     * @return A pair where the first element indicates success (false) or failure (true),
     *         and the second element contains the result message
     */
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

    /**
     * @brief Gets the approved address(es) for a specific token
     * @details Returns both token-specific approvals and operator approvals (approveForAll)
     * @param tokenId The token ID to query
     * @return A pair where the first element indicates success (false) or failure (true),
     *         and the second element contains a vector of approved addresses or an error message
     */
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

    /**
     * @brief Approves or revokes approval for an operator to manage all tokens of the sender
     * @param spender The address to approve or revoke approval for
     * @param approval true to approve, false to revoke approval
     */
    void NonFungibleToken::setApproveForAll(std::string spender, bool approval) {
    Address fromAddr = weilsdk::Runtime::sender();
    std::string key = fromAddr + "$" + EMPTY_TOKEN_ID;

        if (approval) {
            this->getAllowances().insert(key, spender);
        } else {
            this->getAllowances().remove(key);
        }
    }

    /**
     * @brief Checks if an operator is approved to manage all tokens of an owner
     * @param owner The address that owns the tokens
     * @param spender The address to check approval for
     * @return true if the spender is approved, false otherwise
     */
    bool NonFungibleToken::isApprovedForAll(std::string owner,
                                            std::string spender) {
        std::string key = owner + "$" + EMPTY_TOKEN_ID;
        return this->getAllowances().contains(key) &&
               this->getAllowances().get(key) == spender;
    }

    /**
     * @brief Mints a new token with the given ID and metadata
     * @param tokenId The unique ID for the new token
     * @param token The Token object containing the token metadata
     * @return A pair where the first element indicates success (0) or failure (1),
     *         and the second element contains the result message
     */
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
