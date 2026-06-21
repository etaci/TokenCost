import Anthropic from '@anthropic-ai/sdk'
import type { FuseboxConfig } from './types'

interface FuseboxAnthropicConfig extends Omit<Anthropic.ClientOptions, 'baseURL'> {
  fusebox: FuseboxConfig
}

export class FuseboxAnthropic extends Anthropic {
  constructor(config: FuseboxAnthropicConfig) {
    super({
      ...config,
      baseURL: config.fusebox.baseURL,
      defaultHeaders: {
        ...config.defaultHeaders,
        'X-Fusebox-Tenant': config.fusebox.tenant,
      },
    })
  }
}
