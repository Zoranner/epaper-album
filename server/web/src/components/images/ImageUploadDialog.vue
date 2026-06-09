<template>
  <Dialog :open="open" title="上传图片" @close="$emit('close')">
    <form class="dialog-form" @submit.prevent="submit">
      <FileInput label="原始图片" required accept="image/*" @select="selectedFile = $event" />
      <Input
        label="备注"
        :maxlength="120"
        placeholder="例如：海边晚风"
        :model-value="remark"
        @update:model-value="remark = $event"
      />
      <p v-if="error" class="form-error">{{ error }}</p>
      <DialogActions>
        <Button type="button" variant="secondary" @click="$emit('close')">取消</Button>
        <Button :disabled="!selectedFile" :loading="uploading" type="submit" variant="primary">
          上传
        </Button>
      </DialogActions>
    </form>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { uploadImage, type AdminImage } from '../../api';
import Button from '../base/Button.vue';
import FileInput from '../input/FileInput.vue';
import Input from '../input/Input.vue';
import Dialog from '../overlay/Dialog.vue';
import DialogActions from '../overlay/DialogActions.vue';
import { useAuthStore } from '../../composables/useAuthStore';

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
  uploaded: [image: AdminImage];
}>();

const auth = useAuthStore();
const selectedFile = ref<File | null>(null);
const remark = ref('');
const error = ref('');
const uploading = ref(false);

async function submit() {
  if (!selectedFile.value || !auth.token.value) {
    return;
  }

  error.value = '';
  uploading.value = true;
  try {
    const image = await uploadImage(auth.token.value, selectedFile.value, remark.value);
    emit('uploaded', image);
  } catch (uploadError) {
    error.value = uploadError instanceof Error ? uploadError.message : '图片上传失败';
  } finally {
    uploading.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (!open) {
      selectedFile.value = null;
      remark.value = '';
      error.value = '';
    }
  },
);
</script>
