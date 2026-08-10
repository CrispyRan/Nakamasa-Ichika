<template>
  <a-spin :loading="loading" style="width: 100%">
    <a-card class="general-card" :header-style="{ paddingBottom: '14px' }">
      <template #title>热门作者榜单</template>
      <template #extra>
        <a-link>查看更多</a-link>
      </template>
      <a-table
        :data="tableData.list"
        :pagination="false"
        :bordered="false"
        style="margin-bottom: 20px"
        :scroll="{ x: '100%', y: '350px' }"
      >
        <template #columns>
          <a-table-column title="排名" data-index="ranking" />
          <a-table-column title="作者" data-index="author" />
          <a-table-column
            title="内容量"
            data-index="contentCount"
            :sortable="{ sortDirections: ['ascend', 'descend'] }"
          />
          <a-table-column
            title="点击量"
            data-index="clickCount"
            :sortable="{ sortDirections: ['ascend', 'descend'] }"
          />
        </template>
      </a-table>
    </a-card>
  </a-spin>
</template>

<script setup>
import { ref } from 'vue'
import useLoading from '@/hooks/loading'
import { queryPopularAuthor } from '@/api/system/visualization'

const { loading, setLoading } = useLoading(true)
const tableData = ref({ list: [] })

const fetchData = async () => {
  try {
    const res = await queryPopularAuthor()
    if (res.code === 200 && res.data) {
      tableData.value = res.data
    }
  } catch (err) {
    // 模拟数据
    tableData.value = {
      list: [
        { ranking: 1, author: '张三', contentCount: 1234, clickCount: 56789 },
        { ranking: 2, author: '李四', contentCount: 987, clickCount: 45678 },
        { ranking: 3, author: '王五', contentCount: 876, clickCount: 34567 },
        { ranking: 4, author: '赵六', contentCount: 765, clickCount: 23456 },
        { ranking: 5, author: '孙七', contentCount: 654, clickCount: 12345 }
      ]
    }
  } finally {
    setLoading(false)
  }
}

fetchData()
</script>

<style scoped lang="less">
.general-card {
  max-height: 425px;
}
</style>
