// Finds an org's applet by its WNS name (`<kind>::<org>`), e.g. its WeilStream
// publisher, `publisher::<org>`. Needs @weilliptic/weil-sdk 2.6.2 or later
// (for `namehash`), imported by package name through an import map.
import {
  namehash,
  parseExecutionResult,
  parseSchema,
  Schema,
  WeilWalletConnection,
} from '@weilliptic/weil-sdk'

/** The org the wallet has selected for this account, or `null` when none is linked. */
export const getActiveOrg = async (provider, address) => {
  const result = await provider.request({
    method: 'wallet_getOrgsForAccount',
    params: { address },
  })
  return result?.activeOrg?.org || null
}

/**
 * The WNS resolver applet for this account, from the sentinel the wallet is
 * pointed at. The sentinel picks the pod from the account.
 */
const getResolverAddress = async (provider, address) => {
  const { sentinelEndpoint } = await provider.request({
    method: 'wallet_getSettings',
  })
  const base = sentinelEndpoint.replace(/\/+$/, '')
  const response = await fetch(base + '/get_applet_address', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ svc_name: 'wnsresolver', wallet_address: address }),
  })
  const raw = await response.text()
  if (!response.ok) {
    throw new Error(raw.trim() || 'Unable to find the WNS resolver')
  }
  // `{ Ok: address }` or `{ Err: message }`
  const parsed = JSON.parse(raw)
  if (parsed.Err) {
    throw new Error(parsed.Err)
  }
  return parsed.Ok
}

/** The address registered as `<kind>::<tld>`, or `null` when there is none. */
export const resolveKind = async (provider, address, tld, kind) => {
  const wallet = new WeilWalletConnection({ walletProvider: provider })
  const resolver = await getResolverAddress(provider, address)
  const args = parseSchema(
    Schema.args({ name_hash: Schema.string, kind: Schema.string }),
    { name_hash: namehash(tld), kind },
  )
  const result = await wallet.contracts.execute(resolver, 'get_kind', args)
  return parseExecutionResult(result, Schema.string.nullable())
}
