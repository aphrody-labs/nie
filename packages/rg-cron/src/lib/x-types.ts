// Modèle de données renvoyé par la CLI X (JSON). Les champs sont ceux que produit le parseur de la CLI.

export interface Author {
  username: string;
  name: string;
}

/** Média construit depuis `extended_entities`. */
export interface TweetMedia {
  id?: string;
  /** `photo` | `video` | `animated_gif`. */
  type?: string;
  /** MP4 de plus haut débit pour une vidéo, sinon l'image d'aperçu. */
  url?: string;
  preview_url?: string;
  video_url?: string;
  width?: number;
  height?: number;
  expanded_url?: string;
}

export interface Tweet {
  id: string;
  text: string;
  author: Author;
  /** `legacy.user_id_str`, absent des tweets dont l'auteur n'est pas hydraté. */
  author_id?: string;
  /** Format X (« Wed Oct 10 20:19:24 +0000 2018 »), pas ISO 8601. */
  created_at?: string;
  reply_count: number;
  retweet_count: number;
  like_count: number;
  quote_count: number;
  view_count?: number;
  conversation_id?: string;
  in_reply_to_status_id?: string;
  lang?: string;
  is_note_tweet: boolean;
  /** Rempli seulement si la profondeur de citation demandée est > 0. */
  quoted_tweet?: Tweet | null;
  media?: TweetMedia[];
}
