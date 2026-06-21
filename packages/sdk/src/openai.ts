import OpenAI from 'openai'
import type { FuseboxConfig } from './types'

interface FuseboxOpenAIConfig extends Omit<OpenAI.ClientOptions, 'baseURL'> {
  fusebox: FuseboxConfig
}

export class FuseboxOpenAI extends OpenAI {
  constructor(config: FuseboxOpenAIConfig) {
    super({
      ...config,
      baseURL: `${config.fusebox.baseURL}/v1`,
      defaultHeaders: {
        ...config.defaultHeaders,
        'X-Fusebox-Tenant': config.fusebox.tenant,
      },
    })
  }
}
