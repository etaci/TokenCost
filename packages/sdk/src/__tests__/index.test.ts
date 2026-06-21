import { describe, it, expect } from 'vitest'
import { FuseboxClient } from '../src/index'

describe('FuseboxClient', () => {
  it('should create a client with config', () => {
    const client = new FuseboxClient({
      baseURL: 'http://localhost:8080',
      tenant: 'test',
    })

    expect(client).toBeDefined()
  })

  it('should have breaker API', () => {
    const client = new FuseboxClient({
      baseURL: 'http://localhost:8080',
      tenant: 'test',
    })

    expect(client.breaker).toBeDefined()
    expect(client.breaker.getState).toBeInstanceOf(Function)
    expect(client.breaker.reset).toBeInstanceOf(Function)
  })

  it('should have spend API', () => {
    const client = new FuseboxClient({
      baseURL: 'http://localhost:8080',
      tenant: 'test',
    })

    expect(client.spend).toBeDefined()
    expect(client.spend.get).toBeInstanceOf(Function)
  })

  it('should have budget API', () => {
    const client = new FuseboxClient({
      baseURL: 'http://localhost:8080',
      tenant: 'test',
    })

    expect(client.budget).toBeDefined()
    expect(client.budget.requestIncrease).toBeInstanceOf(Function)
    expect(client.budget.listRequests).toBeInstanceOf(Function)
  })

  it('should have events API', () => {
    const client = new FuseboxClient({
      baseURL: 'http://localhost:8080',
      tenant: 'test',
    })

    expect(client.events).toBeDefined()
    expect(client.events.list).toBeInstanceOf(Function)
    expect(client.events.stream).toBeInstanceOf(Function)
  })
})
