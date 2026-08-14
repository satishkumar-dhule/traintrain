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
      if (err instanceof AggregateError && err.errors.length > 0) {
        const firstError = err.errors[0];
        throw new Error(`All ${this.sources.length} sub-agents failed to resolve the query: ${query}. Last error: ${firstError?.message || 'unknown'}`);
      }
      throw new Error(`All ${this.sources.length} sub-agents failed to resolve the query: ${query}`);
    }
  }
}
