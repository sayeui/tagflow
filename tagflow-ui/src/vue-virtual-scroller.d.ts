declare module 'vue-virtual-scroller' {
  import { DefineComponent } from 'vue'

  export const RecycleScroller: DefineComponent<{
    items: any[]
    itemSize: number
    keyField: string
  }>
}
