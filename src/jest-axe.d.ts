declare module 'jest-axe' {
  export interface AxeResult {
    violations: Array<{ id: string; description: string }>;
  }

  export function axe(html: Element | string): Promise<AxeResult>;
}
