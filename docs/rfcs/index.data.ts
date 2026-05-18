import { defineLoader } from 'vitepress'
import { getRfcs, type RfcEntry } from '../.vitepress/rfcs'

declare const data: RfcEntry[]
export { data }

export default defineLoader({
  // Glob is relative to this file (docs/rfcs/), so dev-server HMR fires
  // whenever an RFC's title or status changes.
  watch: ['./rfc-*.md'],
  load(): RfcEntry[] {
    return getRfcs()
  },
})
