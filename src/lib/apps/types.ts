import type {
  SageApp,
  UserSageApp,
} from '@/bindings';

export type UserApp = Extract<SageApp, { kind: 'user' }>;

export function asUserApp(app: UserSageApp): UserApp {
  return {
    kind: 'user',
    ...app,
  };
}