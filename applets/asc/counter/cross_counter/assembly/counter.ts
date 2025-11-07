import { Runtime } from '@weilliptic/weil-sdk-asc/assembly/runtime'
import { Result } from '@weilliptic/weil-sdk-asc/assembly/result'
import { Box } from '@weilliptic/weil-sdk-asc/assembly/primitives'
import { WeilError } from '@weilliptic/weil-sdk-asc/assembly/error'

@json
export class CrossCounterContractState {
  constructor() {}

  fetch_counter_from(contractId: string): Result<Box<u64>, WeilError> {
    return Runtime.callContract<u64>(contractId, 'get_count')
  }

  increment_counter_of(contractId: string): Result<Box<bool>, WeilError> {
    return Runtime.callContract<bool>(contractId, 'increment')
  }
}

@json
export class CrossContractArgs {
  contract_id: string

  constructor() {
    this.contract_id = ''
  }
}
