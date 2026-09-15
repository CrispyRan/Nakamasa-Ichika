import { defineStore } from 'pinia'
import tool from '@/utils/tool'
import { Message } from '@arco-design/web-vue'

const buildTerminalUrl = (commandKey, uuid, extend) => {
  const env = import.meta.env
  const baseURL = env.VITE_APP_BASE_URL
  const terminalUrl = '/app/saipackage/index/terminal'
  return (
    baseURL +
    terminalUrl +
    '?command=' +
    commandKey +
    '&uuid=' +
    uuid +
    '&extend=' +
    extend
  )
}

const getToken = () => {
  const env = import.meta.env
  return tool.local.get(env.VITE_APP_TOKEN_PREFIX)
}

const useTerminalStore = defineStore('terminal', {
  state: () => ({
    show: false,
    taskList: [],
    npmRegistry: 'npm',
    packageManager: 'yarn',
    composerRegistry: 'composer'
  }),

  getters: {
    getState() {
      return { ...this.$state }
    }
  },

  actions: {
    setTaskStatus(idx, status) {
      this.taskList[idx].status = status
      this.setTaskShowMessage(idx, true)
    },
    addTaskMessage(idx, message) {
      this.taskList[idx].message = this.taskList[idx].message.concat(message)
    },
    setTaskShowMessage(idx, val = !this.taskList[idx].showMessage) {
      this.taskList[idx].showMessage = val
    },
    cleanTaskList() {
      this.taskList = []
    },
    taskCompleted(idx) {
      if (typeof this.taskList[idx].callback != 'function') return

      const status = this.taskList[idx].status
      if (status == 5 || status == 6) {
        this.taskList[idx].callback(5)
      } else if (status == 4) {
        this.taskList[idx].callback(4)
      }
    },
    findTaskIdxFromUuid(uuid) {
      for (const key in this.taskList) {
        if (this.taskList[key].uuid == uuid) {
          return parseInt(key)
        }
      }
      return false
    },
    findTaskIdxFromGuess(idx) {
      if (!this.taskList[idx]) {
        let taskKey = -1
        for (const key in this.taskList) {
          if (
            this.taskList[key].status == 2 ||
            this.taskList[key].status == 3
          ) {
            taskKey = parseInt(key)
          }
        }
        return taskKey === -1 ? false : taskKey
      } else {
        return idx
      }
    },
    async startEventSource(taskKey) {
      const that = this
      const url = buildTerminalUrl(
        that.taskList[taskKey].command,
        that.taskList[taskKey].uuid,
        that.taskList[taskKey].extend
      )
      try {
        const response = await fetch(url, {
          headers: {
            Token: getToken()
          }
        })
        if (!response.ok) {
          that.setTaskStatus(taskKey, 5)
          that.taskCompleted(taskKey)
          return
        }
        const reader = response.body.getReader()
        const decoder = new TextDecoder()
        let buffer = ''

        const readStream = async () => {
          while (true) {
            const { done, value } = await reader.read()
            if (done) break
            buffer += decoder.decode(value, { stream: true })
            const lines = buffer.split('\n')
            buffer = lines.pop() || ''
            for (const line of lines) {
              if (!line.startsWith('data: ')) continue
              const dataStr = line.slice(6)
              let data
              try {
                data = JSON.parse(dataStr)
              } catch {
                continue
              }
              if (!data || !data.data) continue

              const taskIdx = that.findTaskIdxFromUuid(data.uuid)
              if (taskIdx === false) continue

              if (data.data == 'exec-error') {
                that.setTaskStatus(taskIdx, 5)
                reader.cancel()
                that.taskCompleted(taskIdx)
                that.startTask()
              } else if (data.data == 'exec-completed') {
                reader.cancel()
                if (that.taskList[taskIdx].status != 4) {
                  that.setTaskStatus(taskIdx, 5)
                }
                that.taskCompleted(taskIdx)
                that.startTask()
              } else if (data.data == 'connection-success') {
                that.setTaskStatus(taskIdx, 3)
              } else if (data.data == 'exec-success') {
                that.setTaskStatus(taskIdx, 4)
              } else {
                that.addTaskMessage(taskIdx, data.data)
              }
            }
          }
        }
        readStream().catch(() => {
          reader.cancel()
          const taskIdx = that.findTaskIdxFromGuess(taskKey)
          if (taskIdx !== false) {
            that.setTaskStatus(taskIdx, 5)
            that.taskCompleted(taskIdx)
          }
        })
      } catch {
        const taskIdx = that.findTaskIdxFromGuess(taskKey)
        if (taskIdx !== false) {
          that.setTaskStatus(taskIdx, 5)
          that.taskCompleted(taskIdx)
        }
      }
    },

    addNodeTask(command, extend = '', callback = () => {}) {
      command =
        command +
        '.' +
        (this.packageManager == 'unknown' ? 'npm' : this.packageManager)
      this.addTask(command, extend, callback)
    },

    addTask(command, extend = '', callback = () => {}) {
      this.taskList = this.taskList.concat({
        uuid: tool.uuid(),
        createTime: tool.dateFormat(),
        status: 1,
        command: command,
        message: [],
        showMessage: false,
        extend: extend,
        callback: callback
      })

      // 检查是否有已经失败的任务
      if (this.show === false) {
        for (const key in this.taskList) {
          if (
            this.taskList[key].status == 5 ||
            this.taskList[key].status == 6
          ) {
            Message.warning({
              content: '任务列表中存在失败的任务',
              duration: 2000
            })
            break
          }
        }
      }

      this.startTask()
    },

    startTask() {
      let taskKey = null

      // 寻找可以开始执行的命令
      for (const key in this.taskList) {
        if (this.taskList[key].status == 1) {
          taskKey = parseInt(key)
          break
        }
        if (this.taskList[key].status == 2 || this.taskList[key].status == 3) {
          break
        }
        if (this.taskList[key].status == 4) {
          continue
        }
        if (this.taskList[key].status == 5 || this.taskList[key].status == 6) {
          continue
        }
      }
      if (taskKey !== null) {
        this.setTaskStatus(taskKey, 2)
        this.startEventSource(taskKey)
      }
    },
    retryTask(idx) {
      this.taskList[idx].message = []
      this.setTaskStatus(idx, 1)
      this.startTask()
    },
    delTask(idx) {
      if (this.taskList[idx].status != 2 && this.taskList[idx].status != 3) {
        this.taskList.splice(idx, 1)
      }
    }
  },

  persist: {
    key: 'storeTerminal'
  }
})

export default useTerminalStore
