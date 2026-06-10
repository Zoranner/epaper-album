import * as authApi from './auth';
import * as imagesApi from './images';
import * as plansApi from './plans';

export * from './auth';
export * from './client';
export * from './images';
export * from './plans';
export * from './types';

export const albumApi = {
  ...authApi,
  ...imagesApi,
  ...plansApi,
};
