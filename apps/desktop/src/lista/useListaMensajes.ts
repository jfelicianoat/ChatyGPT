/**
 * Estado y acciones de la lista de mensajes.
 *
 * La paginación de la conversación (H17: Rust pagina y la vista antepone sin
 * mover la lectura), el seguimiento del scroll y el salto a un resultado de
 * búsqueda (H22) vivían sueltos dentro de `App.tsx`. Aquí viajan juntos, y la
 * tarjeta de mensajes recibe `ReturnType<typeof useListaMensajes>`.
 */
import { useEffect, useLayoutEffect, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from "react";

import {
  progressiveConversationWindow,
  type ConversationView,
  type Loadable
} from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";

/** Página que pide «Mostrar mensajes anteriores» (H17). */
export const EARLIER_MESSAGE_PAGE_SIZE = 50;
const INITIAL_VISIBLE_MESSAGES = 80;

type Opciones = {
  /** Conversación abierta: de ella sale la ventana visible. */
  conversation: Loadable<ConversationView> | null;
  /** Escribe la página anterior antepuesta en la conversación abierta. */
  setConversation: Dispatch<SetStateAction<Loadable<ConversationView> | null>>;
  /** Conversación seleccionada según App: descarta páginas llegadas tarde. */
  selectedConversationRef: RefObject<string | null>;
  /** Señal que pide seguir al último mensaje (cambia con el turno). */
  conversationScrollSignal: string;
  /** Decide si el scroll sigue al último mensaje; lo escriben el envío y la apertura. */
  followConversationScrollRef: RefObject<boolean>;
  /** Mensaje al que saltar al terminar de cargar (H22); lo escribe la búsqueda. */
  pendingMessageFocusRef: RefObject<string | null>;
  /** Aviso de navegación: recoge el fallo de la página anterior (H17). */
  setNavigationError: (message: string) => void;
};

export function useListaMensajes({
  conversation,
  setConversation,
  selectedConversationRef,
  conversationScrollSignal,
  followConversationScrollRef,
  pendingMessageFocusRef,
  setNavigationError
}: Opciones) {
  const messageListRef = useRef<HTMLDivElement>(null);
  const prependScrollRef = useRef<{ scrollHeight: number; scrollTop: number } | null>(null);
  const [messageWindow, setMessageWindow] = useState({
    conversationId: null as string | null,
    limit: INITIAL_VISIBLE_MESSAGES
  });
  const [earlierMessagesBusy, setEarlierMessagesBusy] = useState(false);

  const openConversationId = conversation?.state === "ready" ? conversation.value.id : null;
  const effectiveMessageLimit =
    messageWindow.conversationId === openConversationId
      ? messageWindow.limit
      : INITIAL_VISIBLE_MESSAGES;
  // H17: la paginación la hace Rust. Lo cargado se pinta entero; lo anterior
  // se pide aparte con «Mostrar mensajes anteriores».
  const progressiveMessages =
    conversation?.state === "ready"
      ? progressiveConversationWindow(
          conversation.value.messages,
          Math.max(effectiveMessageLimit, conversation.value.messages.length)
        )
      : { visibleItems: [], hiddenCount: 0 };
  const hasEarlierMessages =
    conversation?.state === "ready" && Boolean(conversation.value.hasEarlierMessages);

  useLayoutEffect(() => {
    const pending = prependScrollRef.current;
    const messageList = messageListRef.current;
    if (!pending || !messageList) return;
    messageList.scrollTop =
      pending.scrollTop + Math.max(0, messageList.scrollHeight - pending.scrollHeight);
    prependScrollRef.current = null;
  }, [effectiveMessageLimit]);

  useLayoutEffect(() => {
    const messageList = messageListRef.current;
    if (!messageList || !followConversationScrollRef.current) return;
    messageList.scrollTop = messageList.scrollHeight;
  }, [conversationScrollSignal]);

  useEffect(() => {
    if (messageWindow.conversationId === openConversationId) return;
    prependScrollRef.current = null;
    setMessageWindow({
      conversationId: openConversationId,
      limit: INITIAL_VISIBLE_MESSAGES
    });
  }, [openConversationId, messageWindow.conversationId]);

  /** Pide a Rust la página anterior y la antepone sin mover la lectura (H17). */
  const revealEarlierMessages = async () => {
    if (conversation?.state !== "ready" || !conversation.value.hasEarlierMessages) return;
    const conversationId = conversation.value.id;
    const first = conversation.value.messages[0];
    if (!first) return;
    const messageList = messageListRef.current;
    if (messageList) {
      prependScrollRef.current = {
        scrollHeight: messageList.scrollHeight,
        scrollTop: messageList.scrollTop
      };
    }
    followConversationScrollRef.current = false;
    setEarlierMessagesBusy(true);
    try {
      const page = await platform.getConversationMessagesBefore(
        conversationId,
        first.sequenceNo,
        EARLIER_MESSAGE_PAGE_SIZE
      );
      if (selectedConversationRef.current !== conversationId) return;
      setConversation((current) =>
        current?.state === "ready" && current.value.id === conversationId
          ? {
              state: "ready",
              value: {
                ...current.value,
                messages: [
                  ...page.messages.filter(
                    (message) => !current.value.messages.some((item) => item.id === message.id)
                  ),
                  ...current.value.messages
                ],
                hasEarlierMessages: page.hasEarlierMessages
              }
            }
          : current
      );
      setMessageWindow((current) => ({
        conversationId,
        limit: current.limit + page.messages.length
      }));
      return page;
    } catch (error) {
      setNavigationError(describeError(error));
      return undefined;
    } finally {
      setEarlierMessagesBusy(false);
    }
  };

  // H22: al abrir un resultado de búsqueda se lleva la vista al mensaje que
  // coincidió, cargando páginas anteriores si hace falta.
  useEffect(() => {
    const target = pendingMessageFocusRef.current;
    if (!target || conversation?.state !== "ready") return;
    const loaded = conversation.value.messages.some((message) => message.id === target);
    if (loaded) {
      pendingMessageFocusRef.current = null;
      followConversationScrollRef.current = false;
      window.requestAnimationFrame(() => {
        const element = document.getElementById(`mensaje-${target}`);
        element?.scrollIntoView?.({ block: "center" });
        element?.classList.add("search-hit");
        window.setTimeout(() => element?.classList.remove("search-hit"), 2_500);
      });
      return;
    }
    if (conversation.value.hasEarlierMessages && !earlierMessagesBusy) {
      void revealEarlierMessages();
    } else if (!conversation.value.hasEarlierMessages) {
      pendingMessageFocusRef.current = null;
    }
  }, [conversation, earlierMessagesBusy]);

  return {
    messageListRef,
    messageWindow,
    earlierMessagesBusy,
    effectiveMessageLimit,
    progressiveMessages,
    hasEarlierMessages,
    revealEarlierMessages
  };
}
