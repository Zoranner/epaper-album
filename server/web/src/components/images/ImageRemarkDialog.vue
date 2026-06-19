<template>
  <Dialog :open="open" title="编辑图片信息" @close="$emit('close')">
    <form v-if="image" class="dialog-form" @submit.prevent="submit">
      <code class="dialog-sha">{{ image.sha256 }}</code>
      <Input
        label="备注"
        :maxlength="120"
        placeholder="未填写备注"
        :model-value="remark"
        @update:model-value="remark = $event"
      />
      <Input
        label="标签"
        :maxlength="120"
        placeholder="例如：家庭 旅行"
        :model-value="tagInput"
        @update:model-value="tagInput = $event"
      />
      <p v-if="error" class="form-error">{{ error }}</p>
      <DialogActions>
        <Button type="button" variant="secondary" @click="$emit('close')">取消</Button>
        <Button :loading="saving" type="submit" variant="primary">保存</Button>
      </DialogActions>
    </form>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { errorMessage, parseTagInput, updateImage, type AdminImage } from '../../api';
import Button from '../base/Button.vue';
import Input from '../input/Input.vue';
import Dialog from '../overlay/Dialog.vue';
import DialogActions from '../overlay/DialogActions.vue';
import { useAuthStore } from '../../composables/useAuthStore';

const props = defineProps<{
  open: boolean;
  image: AdminImage | null;
}>();

const emit = defineEmits<{
  close: [];
  saved: [image: AdminImage];
}>();

const auth = useAuthStore();
const remark = ref('');
const tagInput = ref('');
const error = ref('');
const saving = ref(false);

async function submit() {
  if (!props.image || !auth.token.value) {
    return;
  }

  error.value = '';
  saving.value = true;
  try {
    const image = await updateImage(
      auth.token.value,
      props.image.sha256,
      remark.value,
      parseTagInput(tagInput.value),
    );
    emit('saved', image);
  } catch (saveError) {
    error.value = errorMessage(saveError, '备注保存失败');
  } finally {
    saving.value = false;
  }
}

watch(
  () => props.image,
  (image) => {
    remark.value = image?.remark ?? '';
    tagInput.value = image?.tags.join(' ') ?? '';
    error.value = '';
  },
  { immediate: true },
);
</script>
