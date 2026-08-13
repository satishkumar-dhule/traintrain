import { CaptchaRequiredError } from './errors';

export interface CaptchaContext {
  text: string;
  sessionId: string;
}

export interface DataSource<T> {
  name: string;
  fetch(query: string, captcha?: CaptchaContext): Promise<T>;
}

export class AgentAggregator<T> {
  constructor(private sources: DataSource<T>[]) {}

  /**
   * Fan-out to all registered sub-agents (data sources) concurrently.
   * If targetSource is provided, routes exclusively to that agent (e.g. for solving captchas).
   */
  async execute(query: string, targetSource?: string, captcha?: CaptchaContext): Promise<{ source: string; data: T }> {
    if (this.sources.length === 0) {
      throw new Error("No data sources configured");
    }

    if (targetSource) {
      const source = this.sources.find(s => s.name === targetSource);
      if (!source) throw new Error(`Source ${targetSource} not found`);
      const data = await source.fetch(query, captcha);
      return { source: source.name, data };
    }

    const promises = this.sources.map(async (source) => {
      try {
        const data = await source.fetch(query);
        return { source: source.name, data };
      } catch (err) {
        throw err;
      }
    });

    try {
      return await Promise.any(promises);
    } catch (err: any) {
      // If all fail, check if at least one requested a CAPTCHA
      if (err instanceof AggregateError) {
        const captchaError = err.errors.find(e => e instanceof CaptchaRequiredError || e.name === "CaptchaRequiredError");
        if (captchaError) {
          throw captchaError; // Bubble up the captcha challenge to the user
        }
      }
      throw new Error(`All ${this.sources.length} sub-agents failed to resolve the query: ${query}`);
    }
  }
}
