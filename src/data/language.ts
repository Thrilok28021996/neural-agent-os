/** Supported assistant languages. */
export type AssistantLanguage = 'en' | 'hi' | 'te'

export const languageLabels: Record<AssistantLanguage, string> = {
  en: 'English',
  hi: 'हिन्दी (Hindi)',
  te: 'తెలుగు (Telugu)',
}

export const languageSpeechCodes: Record<AssistantLanguage, string> = {
  en: 'en-US',
  hi: 'hi-IN',
  te: 'te-IN',
}
