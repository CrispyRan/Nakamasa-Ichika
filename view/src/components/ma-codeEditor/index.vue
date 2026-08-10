<template>
  <div class="editor" ref="dom" :style="'width: 100%; height: ' + props.height + 'px'"></div>
</template>

<script setup>
import { onMounted, ref, watch, toRaw, onBeforeUnmount } from 'vue'
import { useAppStore } from '@/store'
import { formatJson } from '@/utils/common'
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/php/php.contribution'
import 'monaco-editor/esm/vs/basic-languages/mysql/mysql.contribution'
import 'monaco-editor/esm/vs/basic-languages/html/html.contribution'
import 'monaco-editor/esm/vs/basic-languages/css/css.contribution'
import 'monaco-editor/esm/vs/editor/contrib/find/browser/findController'

const appStore = useAppStore()

const props = defineProps({
  modelValue: {
    type: [String, Object, Array],
    default: () => ''
  },
  defaultModelValue: {
    type: String,
    default: '',
  },
  valueType: {
    type: String,
    default: 'value'
  },
  miniMap: {
    type: Boolean,
    default: false
  },
  isBind: {
    type: Boolean,
    default: false
  },
  height: {
    type: Number,
    default: 400
  },
  language: {
    type: String,
    default: 'javascript'
  },
  readonly: {
    type: Boolean,
    default: false
  },
  // 新增：主题
  theme: {
    type: String,
    default: ''
  },
  // 新增：智能提示
  suggestions: {
    type: Boolean,
    default: true
  }
})

const emit = defineEmits(['update:modelValue'])
const dom = ref()
let instance = null

// 获取主题
const getTheme = () => {
  if (props.theme) return props.theme
  return appStore.mode === 'light' ? 'vs' : 'vs-dark'
}

// 获取配置
const getOptions = () => ({
  tabSize: 4,
  automaticLayout: true,
  scrollBeyondLastLine: false,
  language: props.language,
  theme: getTheme(),
  autoIndent: 'advanced',
  minimap: { enabled: props.miniMap },
  readOnly: props.readonly,
  folding: true,
  // 智能功能
  quickSuggestions: props.suggestions,
  suggestOnTriggerCharacters: props.suggestions,
  parameterHints: { enabled: props.suggestions },
  hover: { enabled: props.suggestions },
  wordBasedSuggestions: props.suggestions ? 'allDocuments' : 'off',
  // 其他功能
  acceptSuggestionOnCommitCharacter: true,
  acceptSuggestionOnEnter: 'on',
  contextmenu: true,
  formatOnPaste: true,
  formatOnType: true,
  renderLineHighlight: 'all',
  cursorBlinking: 'smooth',
  cursorSmoothCaretAnimation: 'on',
  smoothScrolling: true,
  fontSize: 14,
  lineNumbers: 'on',
  wordWrap: 'on'
})

const initEditorValue = () => {
  if (!instance) return
  
  // 获取当前编辑器值
  const currentValue = instance.getValue()
  const newValue = typeof props.modelValue === 'string' ? props.modelValue : formatJson(props.modelValue)
  
  // 如果值相同，跳过更新（避免光标跳动）
  if (currentValue === newValue) return
  
  // 保存当前光标位置
  const position = instance.getPosition()
  const scrollTop = instance.getScrollTop()
  
  if (props.valueType === 'value' && typeof props.modelValue === 'string') {
    instance.setValue(props.modelValue)
  } else if (props.valueType === 'value' && props.modelValue?._onWillDispose === undefined) {
    instance.setValue(formatJson(props.modelValue))
  } else if (props.modelValue) {
    instance.setModel(toRaw(props.modelValue))
  } else {
    instance.setModel(monaco.editor.createModel(props.defaultModelValue, props.language))
  }
  
  // 恢复光标位置
  if (position) {
    instance.setPosition(position)
    instance.setScrollTop(scrollTop)
  }
}

// 更新主题
const setTheme = (theme) => {
  if (instance) {
    monaco.editor.setTheme(theme || getTheme())
  }
}

// 格式化代码
const formatCode = () => {
  if (instance) {
    instance.getAction('editor.action.formatDocument')?.run()
  }
}

// 插入文本
const insertText = (text) => {
  if (instance) {
    const position = instance.getPosition()
    instance.executeEdits('', [{
      range: new monaco.Range(position.lineNumber, position.column, position.lineNumber, position.column),
      text: text
    }])
    emit('update:modelValue', instance.getValue())
  }
}

// 监听 modelValue 变化
watch(() => props.modelValue, () => {
  initEditorValue()
})

// 监听主题变化
watch(() => props.theme, () => setTheme(props.theme))
watch(() => appStore.mode, () => {
  if (!props.theme) setTheme()
})

onMounted(() => {
  instance = monaco.editor.create(dom.value, getOptions())
  initEditorValue()

  instance.onDidBlurEditorText(() => {
    emit('update:modelValue', toRaw(props.valueType === 'value' ? instance.getValue() : instance.getModel()))
  })

  // 实时更新
  instance.onDidChangeModelContent(() => {
    emit('update:modelValue', instance.getValue())
  })
})

onBeforeUnmount(() => {
  if (instance) {
    instance.dispose()
    instance = null
  }
})

const getInstance = () => instance

defineExpose({ getInstance, initEditorValue, setTheme, formatCode, insertText })
</script>

<style scoped lang="less">
.editor {
  border: 1px solid var(--color-border-2);
  border-radius: 3px;
  background: var(--color-bg-2);
}
</style>