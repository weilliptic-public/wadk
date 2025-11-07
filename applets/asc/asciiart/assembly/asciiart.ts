import {
  NonFungibleToken,
  Token,
} from '@weilliptic/weil-sdk-asc/assembly/contracts/non-fungible'
import { WeilSet } from '@weilliptic/weil-sdk-asc/assembly/collections/set'

@json
export class AsciiArtContractState {
  inner: NonFungibleToken
  controllers: WeilSet<string>

  isController(addr: string): boolean {
    return this.controllers.has(addr)
  }

  constructor(controllers: WeilSet<string>, inner: NonFungibleToken) {
    this.controllers = controllers
    this.inner = inner

    const initialTokens: Array<Token> = [
      new Token(
        'A fish going left!',
        'fish 1',
        'A one line ASCII drawing of a fish',
        '<><',
      ),
      new Token(
        'A fish going right!',
        'fish 2',
        'A one line ASCII drawing of a fish swimming to the right',
        '><>',
      ),
      new Token(
        'A big fish going left!',
        'fish 3',
        'A one line ASCII drawing of a fish swimming to the left',
        "<'))><",
      ),
      new Token(
        'A big fish going right!',
        'fish 4',
        'A one line ASCII drawing of a fish swimming to the right',
        "><(('>",
      ),
      new Token(
        'A Face',
        'face 1',
        'A one line ASCII drawing of a face',
        '(-_-)',
      ),
      new Token(
        'Arms raised',
        'arms 1',
        'A one line ASCII drawing of a person with arms raised',
        '\\o/',
      ),
    ]

    for (let index = 0; index < initialTokens.length; index += 1) {
      this.inner.mint(index.toString(), initialTokens[index])
    }
  }
}
