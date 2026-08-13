import { describe, it, expect } from 'vitest';
import { AgentAggregator, DataSource, CaptchaContext } from './src/modules/core/AgentAggregator';

class SuccessSource implements DataSource<string> {
  name = 'Success';
  async fetch(query: string, captcha?: CaptchaContext): Promise<string> { return 'Data from ' + query; }
}

class FailSource implements DataSource<string> {
  name = 'Fail';
  async fetch(query: string, captcha?: CaptchaContext): Promise<string> { throw new Error('Failed'); }
}

class SlowSuccessSource implements DataSource<string> {
  name = 'Slow';
  async fetch(query: string, captcha?: CaptchaContext): Promise<string> { 
    return new Promise<string>(resolve => setTimeout(() => resolve('Slow ' + query), 100)); 
  }
}

describe('AgentAggregator (Fan-out pattern)', () => {
  it('should return the fastest successful result', async () => {
    const aggregator = new AgentAggregator([new FailSource(), new SlowSuccessSource(), new SuccessSource()]);
    const result = await aggregator.execute('test');
    expect(result.source).toBe('Success');
    expect(result.data).toBe('Data from test');
  });

  it('should throw an error if all sources fail', async () => {
    const aggregator = new AgentAggregator([new FailSource(), new FailSource()]);
    await expect(aggregator.execute('test')).rejects.toThrow(/All 2 sub-agents failed/);
  });
});
