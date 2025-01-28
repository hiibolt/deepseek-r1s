const ws = new WebSocket(baseURL);
const chatContainer = document.getElementById('chatContainer');
const messageInput = document.getElementById('messageInput');

let thinkingBubble = null;
let finalAnswerBubble = null;
let thinking = false;

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.dir(data);
  

  switch(data.event) {
    case 'Thinking':
      thinking = true;
      thinkingBubble = createBubble('thinking');
      thinkingBubble.dataset.rawText = '';
      // Show a placeholder
      thinkingBubble.innerHTML = '<em>[ Thinking ]</em>';

      // Make the thinking bubble collapsible right away:
      makeCollapsible(thinkingBubble);

      chatContainer.appendChild(thinkingBubble);
      break;

    case 'DoneThinking':
      thinking = false;
      // Once "thinking" is done, automatically collapse the bubble
      if (thinkingBubble) {
        collapseBubble(thinkingBubble);
        thinkingBubble = null; // we won't update tokens on this bubble anymore
      }
      break;

    case 'Token':
      // Accumulate tokens in the appropriate bubble
      if (thinking && thinkingBubble) {
        // If still in "thinking" phase, stream into that bubble
        updateMarkdown(thinkingBubble, data.token);
      } else {
        // Otherwise, it's part of the final answer
        if (!finalAnswerBubble) {
          finalAnswerBubble = createBubble('received');
          finalAnswerBubble.dataset.rawText = '';
          chatContainer.appendChild(finalAnswerBubble);
        }
        updateMarkdown(finalAnswerBubble, data.token);
      }
      break;

    case 'Done':
      // Done receiving final answer
      finalAnswerBubble = null;
      break;

    default:
      console.warn('Unknown event type:', data.event);
  }

  // Scroll to bottom after each token
  chatContainer.scrollTop = chatContainer.scrollHeight;
};

// Creates a chat bubble (thinking, received, or sent)
function createBubble(type) {
  const bubble = document.createElement('div');
  bubble.className = `chat-bubble ${type}`;
  return bubble;
}

// Make a bubble collapsible by toggling on click
function makeCollapsible(bubble) {
  bubble.classList.add('collapsible');           // (optional: for styling)
  bubble.dataset.collapsed = 'false';            // track collapse state
  bubble.addEventListener('click', toggleBubble);
}

// Toggle between collapsed and expanded
function toggleBubble(e) {
  // Prevent toggling if user is selecting text or if there's no fullContent stored yet
  const bubble = e.currentTarget;
  if (!bubble.dataset.fullContent) {
    return;
  }

  // If it's currently collapsed, expand it; otherwise, collapse it
  if (bubble.dataset.collapsed === 'true') {
    expandBubble(bubble);
  } else {
    collapseBubble(bubble);
  }
}

// Expand bubble: show full content
function expandBubble(bubble) {
  bubble.innerHTML = bubble.dataset.fullContent;      // restore the full content
  bubble.dataset.collapsed = 'false';

  // Re-run MathJax to render any LaTeX
  MathJax.typesetPromise([bubble]).catch(err => console.error(err));
}

// Collapse bubble: hide the full content behind a placeholder
function collapseBubble(bubble) {
  // If we haven't saved the final rendered content, do so now
  // This ensures partial tokens are not lost
  if (!bubble.dataset.fullContent) {
    bubble.dataset.fullContent = bubble.innerHTML;
  }

  // Show a minimal placeholder
  bubble.innerHTML = '[ Click to view hidden thinking ]';
  bubble.dataset.collapsed = 'true';
}

// Accumulate raw Markdown, parse with Marked, then do MathJax
function updateMarkdown(bubble, newText) {
  const currentRaw = bubble.dataset.rawText || '';
  // 1) Accumulate raw text
  bubble.dataset.rawText = currentRaw + newText;

  // 2) Parse to HTML
  const unsafeHTML = marked.parse(bubble.dataset.rawText);
  const safeHTML = DOMPurify.sanitize(unsafeHTML);

  bubble.innerHTML = safeHTML;

  // 3) Re-run MathJax for *this bubble only*
  MathJax.typesetPromise([bubble])
    .then(() => {
      // If the bubble is collapsible (i.e. the "thinking" bubble),
      // update the bubble.dataset.fullContent so the final collapsed state can restore everything
      if (bubble.classList.contains('collapsible') && bubble.dataset.collapsed === 'false') {
        bubble.dataset.fullContent = bubble.innerHTML;
      }
    })
    .catch((err) => console.error(err));
}

// Send the user's message, also rendered as Markdown (and LaTeX!)
function sendMessage() {
  const message = messageInput.value.trim();
  if (!message) return;

  // Create a bubble for the user's message
  const sentBubble = createBubble('sent');

  // Parse the user's message as Markdown
  const unsafeHTML = marked.parse(message);
  const safeHTML = DOMPurify.sanitize(unsafeHTML);
  sentBubble.innerHTML = safeHTML;

  // Append it first, so the user sees it right away
  chatContainer.appendChild(sentBubble);

  // Typeset any math in the user's message
  MathJax.typesetPromise([sentBubble]).catch((err) => console.error(err));

  // Send to the WebSocket server
  ws.send(message);

  // Clear the input
  messageInput.value = '';
  chatContainer.scrollTop = chatContainer.scrollHeight;
}

// Allow "Enter" to send
messageInput.addEventListener('keypress', (e) => {
  if (e.key === 'Enter') {
    sendMessage();
  }
});
