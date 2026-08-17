import { invoke } from './invoke'

export function storeProviderSecret(provider: string, secretKind: string, value: string) {
  return invoke('store_provider_secret', { provider, secretKind, value })
}

export function deleteProviderSecret(provider: string, secretKind: string) {
  return invoke('delete_provider_secret', { provider, secretKind })
}
