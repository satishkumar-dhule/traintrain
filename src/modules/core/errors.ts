export class CaptchaRequiredError extends Error {
  constructor(
    public source: string,
    public imageBase64: string, // Base64 Data URI or URL
    public sessionId: string
  ) {
    super(`Captcha required by ${source}`);
    this.name = 'CaptchaRequiredError';
  }
}
