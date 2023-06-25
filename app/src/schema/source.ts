import {z} from "zod";

export const GetSourcesSchema = z.string().array();
export type GetSources = z.infer<typeof GetSourcesSchema>;
