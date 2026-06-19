let openDialogCount = 0;
let previousBodyOverflow = '';

export function lockDialogScroll() {
  if (openDialogCount === 0) {
    previousBodyOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
  }
  openDialogCount += 1;
}

export function unlockDialogScroll() {
  openDialogCount = Math.max(0, openDialogCount - 1);
  if (openDialogCount === 0) {
    document.body.style.overflow = previousBodyOverflow;
  }
}
