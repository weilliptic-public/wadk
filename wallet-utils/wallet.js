// Weil Wallet widget: state, active account and connect/disconnect, in
// #wallet. Also publishes each chat message as an audit receipt to WeilStream,
// and shows whether receipts can be published. A module, so it shares no names
// with the page; the page reaches it through `window.weilAudit`.
import { WeilWalletConnection } from '@weilliptic/weil-sdk'
import { WeilStreamPublisher } from './publisher.js'
import { getActiveOrg, resolveKind } from './wns.js'

// Receipts go to the active org's WeilStream publisher, `publisher::<org>` in WNS.
const AUDIT_TOPIC = 'audits'
const AUDIT_SOURCE = 'agentic-memory'

const root = document.getElementById('wallet')

const LABELS = {
  pending: 'wallet: detecting...',
  'not-installed': 'wallet: not installed',
  'not-setup': 'wallet: not set up',
  locked: 'wallet: locked',
  disconnected: 'wallet: not connected',
  connected: 'wallet: connected',
  error: 'wallet: error',
}

// Why receipts are unavailable in each wallet state other than `connected`.
const WALLET_REASONS = {
  pending: 'Looking for the Weil Wallet extension.',
  'not-installed': 'The Weil Wallet extension is not installed.',
  'not-setup': 'The wallet has no account yet.',
  locked: 'The wallet is locked.',
  disconnected: 'This site is not connected to the wallet.',
  error: 'The wallet did not answer as expected.',
}

const ICONS = {
  // A seal with a check mark.
  ready:
    '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 2l2.4 1.8 3-.2.9 2.9 2.5 1.7-1 2.8 1 2.8-2.5 1.7-.9 2.9-3-.2L12 22l-2.4-1.8-3 .2-.9-2.9-2.5-1.7 1-2.8-1-2.8 2.5-1.7.9-2.9 3 .2z"/><path d="M8.5 12l2.5 2.5 4.5-5"/></svg>',
  // A warning triangle.
  unavailable:
    '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>',
}
ICONS.checking = ICONS.ready

const findWallet = () =>
  window.WeilWallet && window.WeilWallet.isWeilWallet ? window.WeilWallet : null

// Wallet errors arrive as plain objects ({ message, errorCode }).
const errorMessage = error => (error && error.message) || String(error)
const shorten = address => address.slice(0, 6) + '...' + address.slice(-4)

// SHA-256 of the message's UTF-8 bytes, as lowercase hex.
async function sha256Hex(text) {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(text),
  )
  return Array.from(new Uint8Array(digest), b =>
    b.toString(16).padStart(2, '0'),
  ).join('')
}

let wallet = null
let busy = false
let current = { state: 'pending', accounts: [] }
// Whether receipts can be published right now, and if not, why.
let receipts = { status: 'checking', reasons: [] }
// The outcome of the latest publish, shown next to the account.
let lastAudit = null
// `account|org` -> a receipts check that came back ready, so WNS and the topic
// are checked once per account and org. Anything else is checked again on the
// next refresh, so a topic created meanwhile is picked up.
const checks = new Map()
// Drops a receipts check that finishes after a newer one has started.
let checkRun = 0

function button(label, onClick) {
  const b = document.createElement('button')
  b.type = 'button'
  b.textContent = label
  b.disabled = busy
  b.onclick = onClick
  return b
}

function receiptsIcon() {
  const icon = document.createElement('span')
  icon.className = 'receipts'
  icon.dataset.status = receipts.status
  icon.innerHTML = ICONS[receipts.status]

  let tooltip
  if (receipts.status === 'ready') {
    tooltip =
      'Audits ready\n' +
      'Each message is published to the "' +
      AUDIT_TOPIC +
      '" topic of publisher::' +
      receipts.org
    const label = document.createElement('span')
    label.textContent = 'audits'
    icon.append(label)
  } else if (receipts.status === 'checking') {
    tooltip = 'Checking whether receipts can be published...'
  } else {
    tooltip =
      'Receipts unavailable:\n' + receipts.reasons.map(r => '• ' + r).join('\n')
  }
  icon.title = tooltip
  icon.setAttribute('role', 'img')
  icon.setAttribute('aria-label', tooltip)
  return icon
}

function auditStatus() {
  const status = document.createElement('span')
  status.className = 'account'
  status.textContent = lastAudit.text
  if (lastAudit.detail) status.title = lastAudit.detail
  return status
}

function render(state, accounts, problem) {
  current = { state, accounts }

  const badge = document.createElement('span')
  badge.className = 'badge'
  badge.dataset.state = state
  badge.textContent = LABELS[state] || 'wallet: ' + state
  if (problem) badge.title = problem

  const parts = [badge, receiptsIcon()]
  const active = accounts.find(a => a.active) || accounts[0]

  if (state === 'connected' && active) {
    const account = document.createElement('span')
    account.className = 'account'
    account.title = accounts.map(a => a.name + '  ' + a.address).join('\n')
    account.textContent =
      active.name +
      ' ' +
      shorten(active.address) +
      (accounts.length > 1 ? ' +' + (accounts.length - 1) : '')
    parts.push(account)
    if (lastAudit) parts.push(auditStatus())
    parts.push(button('Disconnect', disconnect))
  } else if (state === 'disconnected') {
    parts.push(button('Connect', connect))
  } else if (state === 'locked') {
    parts.push(button('Unlock', unlock))
  } else if (state === 'not-setup') {
    // `wallet_open` opens the wallet in a tab but never answers.
    parts.push(
      button('Set up', () => wallet.request({ method: 'wallet_open' })),
    )
  }

  root.replaceChildren(...parts)
}

const rerender = () => render(current.state, current.accounts)

/**
 * Everything a receipt needs, beyond a connected wallet: an org on the active
 * account, that org's publisher in WNS, and the audit topic on the publisher.
 */
async function checkReceipts(account) {
  const org = await getActiveOrg(wallet, account.address)
  if (!org) {
    return {
      status: 'unavailable',
      reasons: ['Link an organization from your wallet account.'],
    }
  }

  const key = account.address + '|' + org
  if (!checks.has(key)) {
    const check = checkPublisher(account, org)
    checks.set(key, check)
    check.then(result => {
      if (result.status !== 'ready') checks.delete(key)
    })
  }
  return checks.get(key)
}

async function checkPublisher(account, org) {
  const name = 'publisher::' + org
  let address
  try {
    address = await resolveKind(wallet, account.address, org, 'publisher')
  } catch (error) {
    return {
      status: 'unavailable',
      reasons: ['Could not look up ' + name + ': ' + errorMessage(error)],
    }
  }
  if (!address) {
    return {
      status: 'unavailable',
      reasons: [name + ' is not registered in WNS.'],
    }
  }

  const publisher = WeilStreamPublisher(
    new WeilWalletConnection({ walletProvider: wallet }),
    address,
  )
  try {
    await publisher.topic_info(AUDIT_TOPIC)
  } catch (error) {
    return {
      status: 'unavailable',
      reasons: [
        'The "' +
          AUDIT_TOPIC +
          '" topic is not available on ' +
          name +
          ': ' +
          errorMessage(error),
      ],
    }
  }

  return { status: 'ready', reasons: [], org, publisher }
}

async function updateReceipts() {
  const run = ++checkRun
  const account = current.accounts.find(a => a.active)

  if (current.state !== 'connected' || !account) {
    receipts = {
      status: current.state === 'pending' ? 'checking' : 'unavailable',
      reasons: [
        WALLET_REASONS[current.state] || 'The wallet is not connected.',
      ],
      fromWallet: true,
    }
    return rerender()
  }

  // Just connected: the previous reason no longer applies.
  if (receipts.fromWallet) {
    receipts = { status: 'checking', reasons: [] }
    rerender()
  }

  // Keeps showing the previous result while checking again, rather than
  // flickering on every focus.
  let result
  try {
    result = await checkReceipts(account)
  } catch (error) {
    result = { status: 'unavailable', reasons: [errorMessage(error)] }
  }
  if (run !== checkRun) return
  receipts = result
  rerender()
}

// The wallet sends no event on lock or unlock, so this also runs when the tab
// regains focus.
async function refresh() {
  if (!wallet) {
    render('not-installed', [])
  } else {
    try {
      if (!(await wallet.isSetUp())) {
        render('not-setup', [])
      } else if (!(await wallet.isUnlocked())) {
        render('locked', [])
      } else {
        // Only the accounts this site is connected to.
        const accounts = await wallet.request({ method: 'weil_accounts' })
        render(accounts.length ? 'connected' : 'disconnected', accounts)
      }
    } catch (error) {
      if (error && error.errorCode === 'wallet-locked') render('locked', [])
      else render('error', [], errorMessage(error))
    }
  }
  updateReceipts()
}

// Runs a wallet request with the buttons disabled, then re-reads state.
async function act(method) {
  busy = true
  rerender()
  try {
    await wallet.request({ method })
  } catch (error) {
    // Closing the approval popup lands here; the state shows the outcome.
    console.info('Weil Wallet ' + method + ': ' + errorMessage(error))
  } finally {
    busy = false
    refresh()
  }
}

const connect = () => act('weil_requestAccounts')
const unlock = () => act('wallet_unlock')
const disconnect = () => act('wallet_disconnect')

/**
 * Publishes a chat message to the audit topic, signed by the active account.
 * Never throws: the outcome is shown in the widget, and a failed receipt must
 * not break the chat.
 */
async function publishAudit(message) {
  if (receipts.status !== 'ready') {
    console.info('Weil receipt skipped: ' + receipts.reasons.join(' '))
    return
  }

  lastAudit = { text: 'receipt: publishing...' }
  rerender()

  try {
    const hash = await sha256Hex(message)
    const receipt = await receipts.publisher.publish(
      AUDIT_TOPIC,
      JSON.stringify({ message, hash, source: AUDIT_SOURCE }),
    )
    lastAudit = {
      text: 'receipt #' + receipt.seq,
      detail:
        'publisher::' +
        receipts.org +
        ', topic ' +
        receipt.topic +
        ', txn ' +
        receipt.txn_id,
    }
  } catch (error) {
    lastAudit = { text: 'receipt: failed', detail: errorMessage(error) }
    console.warn('Weil receipt failed: ' + errorMessage(error))
    // Whatever was ready may not be any more; check again.
    checks.clear()
    updateReceipts()
  } finally {
    rerender()
  }
}

window.weilAudit = { publish: publishAudit }

function attach(found) {
  wallet = found
  // The only events the wallet sends a page; the callback gets the payload.
  // Either can change which publisher the next receipt goes to.
  wallet.on('switchAccount', () => {
    checks.clear()
    refresh()
  })
  wallet.on('switchOrg', () => {
    checks.clear()
    refresh()
  })
  refresh()
}

window.addEventListener('focus', refresh)
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') refresh()
})

render('pending', [])

// The extension injects `window.WeilWallet` shortly after the page starts, and
// may be enabled while it is open, so keep looking.
const started = Date.now()
let shownMissing = false
const timer = setInterval(() => {
  const found = findWallet()
  if (found) {
    clearInterval(timer)
    attach(found)
  } else if (!shownMissing && Date.now() - started > 3000) {
    shownMissing = true
    render('not-installed', [])
    updateReceipts()
  }
}, 100)
