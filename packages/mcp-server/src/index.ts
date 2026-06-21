#!/usr/bin/env node
/**
 * Fusebox MCP Server
 *
 * Exposes Fusebox monitoring and budget management capabilities to AI agents
 * via the Model Context Protocol (MCP).
 *
 * Tools provided:
 * - get_budget: Check current budget configuration
 * - get_spend: View spend in a time window
 * - get_breaker: Check circuit breaker state
 * - request_budget_increase: Request a budget increase
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';

const FUSEBOX_URL = process.env.FUSEBOX_URL || 'http://localhost:8080';
const DEFAULT_TENANT = process.env.FUSEBOX_TENANT || 'default';

// Zod schemas for tool arguments
const GetBudgetArgsSchema = z.object({
  tenant: z.string().optional().describe('Tenant ID (defaults to FUSEBOX_TENANT env var)'),
});

const GetSpendArgsSchema = z.object({
  tenant: z.string().optional().describe('Tenant ID'),
  window: z.enum(['1m', '1h', '1d', '1w', '1mo']).default('1d').describe('Time window'),
});

const GetBreakerArgsSchema = z.object({
  tenant: z.string().optional().describe('Tenant ID'),
});

const RequestBudgetIncreaseArgsSchema = z.object({
  tenant: z.string().optional().describe('Tenant ID'),
  limit_usd: z.number().positive().describe('Requested budget limit in USD'),
  window: z.enum(['minute', 'hour', 'day', 'week', 'month']).describe('Budget window'),
  reason: z.string().optional().describe('Reason for the increase'),
  ttl_seconds: z.number().optional().describe('TTL in seconds (if approved)'),
});

/**
 * Fusebox API client
 */
class FuseboxClient {
  constructor(private baseUrl: string) {}

  private async fetch(path: string, options?: RequestInit): Promise<any> {
    const url = `${this.baseUrl}${path}`;
    const response = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
    });

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Fusebox API error: ${response.status} ${text}`);
    }

    return response.json();
  }

  async getBudget(tenant: string) {
    // Fusebox doesn't have a direct budget GET endpoint yet,
    // so we derive it from the spend endpoint which includes budget info
    return this.fetch(`/v1/spend?tenant=${encodeURIComponent(tenant)}`);
  }

  async getSpend(tenant: string, window: string) {
    return this.fetch(`/v1/spend?tenant=${encodeURIComponent(tenant)}&window=${window}`);
  }

  async getBreaker(tenant: string) {
    return this.fetch(`/v1/breaker/state?tenant=${encodeURIComponent(tenant)}`);
  }

  async requestBudgetIncrease(params: {
    tenant: string;
    limit_usd: number;
    window: string;
    reason?: string;
    ttl_seconds?: number;
  }) {
    return this.fetch('/v1/budget/requests', {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }
}

/**
 * MCP Server implementation
 */
class FuseboxMcpServer {
  private server: Server;
  private client: FuseboxClient;

  constructor() {
    this.server = new Server(
      {
        name: 'fusebox-mcp-server',
        version: '0.1.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.client = new FuseboxClient(FUSEBOX_URL);
    this.setupHandlers();
  }

  private setupHandlers() {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      return {
        tools: this.getTools(),
      };
    });

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      try {
        switch (name) {
          case 'get_budget':
            return await this.handleGetBudget(args);
          case 'get_spend':
            return await this.handleGetSpend(args);
          case 'get_breaker':
            return await this.handleGetBreaker(args);
          case 'request_budget_increase':
            return await this.handleRequestBudgetIncrease(args);
          default:
            throw new Error(`Unknown tool: ${name}`);
        }
      } catch (error) {
        return {
          content: [
            {
              type: 'text',
              text: `Error: ${error instanceof Error ? error.message : String(error)}`,
            },
          ],
          isError: true,
        };
      }
    });
  }

  private getTools(): Tool[] {
    return [
      {
        name: 'get_budget',
        description:
          'Get the current budget configuration for a tenant. Returns the budget limit and window.',
        inputSchema: {
          type: 'object',
          properties: {
            tenant: {
              type: 'string',
              description: 'Tenant ID (defaults to FUSEBOX_TENANT env var)',
            },
          },
        },
      },
      {
        name: 'get_spend',
        description:
          'Get current spending for a tenant in a specific time window. Returns cost, token counts, and budget status.',
        inputSchema: {
          type: 'object',
          properties: {
            tenant: {
              type: 'string',
              description: 'Tenant ID',
            },
            window: {
              type: 'string',
              enum: ['1m', '1h', '1d', '1w', '1mo'],
              description: 'Time window (default: 1d)',
            },
          },
        },
      },
      {
        name: 'get_breaker',
        description:
          'Check the circuit breaker state for a tenant. Returns whether the breaker is open, closed, or half-open.',
        inputSchema: {
          type: 'object',
          properties: {
            tenant: {
              type: 'string',
              description: 'Tenant ID',
            },
          },
        },
      },
      {
        name: 'request_budget_increase',
        description:
          'Request a budget increase for a tenant. Creates a pending request that must be approved by an admin.',
        inputSchema: {
          type: 'object',
          properties: {
            tenant: {
              type: 'string',
              description: 'Tenant ID',
            },
            limit_usd: {
              type: 'number',
              description: 'Requested budget limit in USD',
            },
            window: {
              type: 'string',
              enum: ['minute', 'hour', 'day', 'week', 'month'],
              description: 'Budget window',
            },
            reason: {
              type: 'string',
              description: 'Reason for the increase (optional)',
            },
            ttl_seconds: {
              type: 'number',
              description: 'TTL in seconds if approved (optional)',
            },
          },
          required: ['limit_usd', 'window'],
        },
      },
    ];
  }

  private async handleGetBudget(args: unknown) {
    const parsed = GetBudgetArgsSchema.parse(args);
    const tenant = parsed.tenant || DEFAULT_TENANT;
    const data = await this.client.getBudget(tenant);

    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(data, null, 2),
        },
      ],
    };
  }

  private async handleGetSpend(args: unknown) {
    const parsed = GetSpendArgsSchema.parse(args);
    const tenant = parsed.tenant || DEFAULT_TENANT;
    const data = await this.client.getSpend(tenant, parsed.window);

    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(data, null, 2),
        },
      ],
    };
  }

  private async handleGetBreaker(args: unknown) {
    const parsed = GetBreakerArgsSchema.parse(args);
    const tenant = parsed.tenant || DEFAULT_TENANT;
    const data = await this.client.getBreaker(tenant);

    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(data, null, 2),
        },
      ],
    };
  }

  private async handleRequestBudgetIncrease(args: unknown) {
    const parsed = RequestBudgetIncreaseArgsSchema.parse(args);
    const tenant = parsed.tenant || DEFAULT_TENANT;

    const data = await this.client.requestBudgetIncrease({
      tenant,
      limit_usd: parsed.limit_usd,
      window: parsed.window,
      reason: parsed.reason,
      ttl_seconds: parsed.ttl_seconds,
    });

    return {
      content: [
        {
          type: 'text',
          text: `Budget increase request created: ${JSON.stringify(data, null, 2)}`,
        },
      ],
    };
  }

  async run() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Fusebox MCP Server running on stdio');
  }
}

// Start the server
const server = new FuseboxMcpServer();
server.run().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
